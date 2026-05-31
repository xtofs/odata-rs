---
description: These are the general architectural instructions that apply to all major refactoring approaches
# applyTo: 'Describe when these instructions should be loaded by the agent based on task context' # when provided, instructions will automatically be added to the request context when the pattern matches an attached file
---

**OData in Rust**

Architecture Overview

**Context and motivation**

ASP.NET's OData library is architecturally built around IQueryable\<T\>
and Entity Framework. That pairing is not incidental: IQueryable is the
load-bearing abstraction that lets the library defer execution, compose
expressions, and hand query trees to a provider. Remove it and you
remove the design's foundation, not a convenience layer.

Rust has none of these constraints. There is no IQueryable analogue, no
mandated data-access layer equivalent to ADO/System.Data, and no ambient
async runtime. This is liberating: a Rust OData library can be designed
around the right abstraction rather than an inherited one.

**Core design decision: who owns execution?**

In .NET, IQueryable implicitly answers this: whoever receives the value
executes it. The library smuggles expression trees across an API
boundary and relies on a provider (EF) to compile them. In Rust, the
answer is explicit and enforced by the type system:

**The library owns query representation. You own execution.**

This is expressed through a single trait. The library defines the
contract; every backend (Postgres via sqlx, SQLite, an in-memory Vec, a
remote API) implements it independently.

**The ODataSource trait**

> trait ODataSource {
>
> type Entity: Serialize;
>
> type Error;
>
> async fn execute(
>
> &self,
>
> query: ODataQuery,
>
> ) -\> Result\<ODataResponse\<Self::Entity\>, Self::Error\>;
>
> }

ODataQuery is pure data --- a parsed, typed representation of the URL
with no behavior attached. The library never inspects your data layer;
it hands you a structured query and expects a structured response. Swap
backends by swapping the impl.

**Query composition**

OData queries compose naturally as value transformations. Tenant
scoping, permission filtering, and field redaction are all
QueryTransform --- plain functions from ODataQuery to ODataQuery ---
that can be tested independently, logged, and piped:

> let query = parse_odata_url(&req)?
>
> .pipe(require_tenant(ctx.tenant_id))
>
> .pipe(apply_permission_filter(&ctx.user));

In .NET this is done by mutating IQueryable at controller level:
implicit, stateful, and impossible to test without a running DB context.
The Rust approach makes transformations explicit values.

**\$expand without navigation properties**

Without an ORM graph, \$expand becomes explicit. The library provides an
Expandable\<Relation\> trait; you implement batch-fetch-and-stitch per
relation. This naturally aligns with a DataLoader pattern and avoids N+1
without framework magic.

**Layer summary**

  --------------- ----------------------- -----------------------
  **Layer**       **Content**             **Responsibility
                                          boundary**

  Parser          URL → ODataQuery AST    You provide this (crate
                                          or hand-rolled)

  EDM             Type registration,      You provide this
                  \$metadata              

  ODataSource     execute(&self,          Library defines; you
  trait           ODataQuery) →           implement per backend
                  Result\<...\>           

  Composition     FilterExpr combinators, Library provides
  utils           QueryTransform pipeline 

  Serialization   OData JSON envelope,    Library provides
                  \@odata.count, nextLink 
  --------------- ----------------------- -----------------------

*Parser and EDM layers are caller-supplied. The library scaffolds
ODataSource, composition utilities, and serialization.*
