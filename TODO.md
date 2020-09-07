# Limitations
* Type aliases (`pub type Foo = Bar;`) and re-exports (`pub use foo::Bar`) are not shown.
    * Unsure how to get them in the analysis data.
* Visibility info (pub / pub(crate) / not pub / etc) is not shown.
    * Probably not possible with current analysis data.
    * Could maybe hack it by parsing the source code span and/or code around the span?
* Types get shown with their canonical name (after resolving all aliases and re-exports) regardless of what the code called them. So you'll see lots of `impl core::...` when the code really wrote `impl std::...` because `std` re-exports lots of things from `core`.
    * Probably not possible to fix this without parsing the source code.
* It would be nice to show crate versions, but versions are a Cargo thing, not a rustc thing, and so it isn't present in the analysis data anywhere.

# Enhancements
* format the label of functions to include the signature?
    * probably will invole string munging, sticking the `.name` into the the `.value` of the def in the right spot
    * might make it hard to read though, because argument lists can be very long.
* implement some form of live search, where you can start typing and rsbrowse selects the thing
    * initially, within the current pane would be a nice start
    * eventually, within the current crate is probably good enough
    * globally is probably a bad idea
    * let users hit F3 or something to continue to the next match
* Allow some way to specify a particular rust toolchain / target. Currently we just run `rustc` and `cargo` and you get whatever the default is.
* in struct definitions, fields should be expandable to their type
