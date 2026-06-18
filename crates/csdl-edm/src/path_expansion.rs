//! Path expansion support used to demonstrate traversal over the resolved
//! EDM semantic model (not the raw CSDL syntax model).

use std::sync::Arc;

use crate::edm::{EntityContainerElement, EntityType, Model};

#[derive(Debug, Clone)]
pub struct SpawnedPath {
    pub segments: Vec<String>,
    pub terminal_type: Arc<EntityType>,
    pub is_collection: bool,
}

pub struct PathExpander {
    max_key_segments: usize,
}

impl PathExpander {
    pub fn new(max_key_segments: usize) -> Self {
        Self { max_key_segments }
    }

    pub fn collect_paths(&self, model: &Model) -> Vec<SpawnedPath> {
        let mut paths = Vec::new();
        let Some(container) = &model.entity_container else {
            return paths;
        };

        for el in &container.elements {
            match el.as_ref() {
                EntityContainerElement::EntitySet(es) => {
                    let current = vec![es.name.clone()];
                    paths.push(SpawnedPath {
                        segments: current.clone(),
                        terminal_type: es.target.clone(),
                        is_collection: true,
                    });
                    //

                    let (instance_segments, next_key_segments, key_added, can_descend) =
                        expand_collection_path(current, &es.target, 0, self.max_key_segments);

                    if key_added {
                        paths.push(SpawnedPath {
                            segments: instance_segments.clone(),
                            terminal_type: es.target.clone(),
                            is_collection: false,
                        });
                    }

                    if key_added && can_descend {
                        self.collect_nav_paths(
                            &es.target,
                            instance_segments,
                            next_key_segments,
                            &mut paths,
                        );
                    }
                }
                EntityContainerElement::Singleton(s) => {
                    let current = vec![s.name.clone()];
                    paths.push(SpawnedPath {
                        segments: current.clone(),
                        terminal_type: s.target.clone(),
                        is_collection: false,
                    });
                    self.collect_nav_paths(&s.target, current, 0, &mut paths);
                }
                EntityContainerElement::FunctionImport(_)
                | EntityContainerElement::ActionImport(_) => {
                    // Imports do not provide an entity target path root.
                }
            }
        }

        paths
    }

    fn collect_nav_paths(
        &self,
        entity: &Arc<EntityType>,
        current_segments: Vec<String>,
        key_segments: usize,
        paths: &mut Vec<SpawnedPath>,
    ) {
        struct Frame {
            entity: Arc<EntityType>,
            segments: Vec<String>,
            key_segments: usize,
            next_nav_index: usize,
        }

        let mut stack = vec![Frame {
            entity: entity.clone(),
            segments: current_segments,
            key_segments,
            next_nav_index: 0,
        }];

        while let Some(mut frame) = stack.pop() {
            let navs = frame.entity.navigation_properties();
            if frame.next_nav_index >= navs.len() {
                continue;
            }

            let nav = navs[frame.next_nav_index].clone();
            let parent_segments = frame.segments.clone();
            let parent_key_segments = frame.key_segments;
            frame.next_nav_index += 1;
            stack.push(frame);

            let Some(target) = nav.target.upgrade() else {
                continue;
            };

            let mut next_segments = parent_segments;
            next_segments.push(nav.name.clone());

            let mut next_key_segments = parent_key_segments;
            let mut can_descend = true;
            let mut ends_in_collection = nav.is_collection;
            if nav.is_collection {
                let (expanded, expanded_keys, key_added, expanded_can_descend) =
                    expand_collection_path(
                        next_segments,
                        &target,
                        parent_key_segments,
                        self.max_key_segments,
                    );
                next_segments = expanded;
                next_key_segments = expanded_keys;
                can_descend = expanded_can_descend;
                ends_in_collection = !key_added;
            }

            paths.push(SpawnedPath {
                segments: next_segments.clone(),
                terminal_type: target.clone(),
                is_collection: ends_in_collection,
            });

            if can_descend && next_key_segments < self.max_key_segments {
                stack.push(Frame {
                    entity: target,
                    segments: next_segments,
                    key_segments: next_key_segments,
                    next_nav_index: 0,
                });
            }
        }
    }
}

fn expand_collection_path(
    mut segments: Vec<String>,
    target: &Arc<EntityType>,
    key_segments: usize,
    max_key_segments: usize,
) -> (Vec<String>, usize, bool, bool) {
    let Some(first_key) = target.keys().first() else {
        return (segments, key_segments, false, false);
    };

    if key_segments + 1 < max_key_segments {
        segments.push(format!("{{{}}}", crate::edm::key_path_to_string(first_key)));
        return (segments, key_segments + 1, true, true);
    }

    // Emit the collection segment itself, but do not append a key segment
    // that would violate the exclusive key-segment limit.
    (segments, key_segments, false, false)
}
