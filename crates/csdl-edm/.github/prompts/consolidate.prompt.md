---
name: consolidate
description: consolidate two related projects
---
name: 
description: there are two project in the vscode workspace workspace that have developed somewhat independently.
/Users/christof/code/csdl-edm/Cargo.toml
/Users/christof/code/temp/Cargo.toml

## the csdl-edm folder has
- mature architecture with separation of syntactical (csdl) and semantic model (edm) 
- ergonomic semantic model that allows to traverse the edm graph and leveraged rusts' Arc and Weak pointers effectively
- a resolver to transform an csdl model to an edm model
- a separate validator the checks the edm model against additional semantic rules from the standard

## the temp folder
- a more complete csdl model covering more of the standards csdl elements
- parsers for both json and xml 
- a efficient separation between csdl (CML and JSON) readers vs parser where the reader normalizes certain constructs to make the paring streamlined
- a more complete view of the annotation expressions domain
- serialization of the csdl model into XML and JSON.


I want to to merge the two creates into one, using the best of the implementation when there are two versions of it and ultimately creating a single create that covers JSON and XML CSDL parsing and serialization, transformation into EDM (resolution) and validation.

the create should then also contain the main programs from both as rust examples since they both shows aspects of the architecture 

TODO and Architecture descriptions should also be preserved and consolidated in their respective folders.
