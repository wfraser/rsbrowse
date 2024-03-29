# Limitations
* Types get shown with their canonical name (after resolving all aliases and re-exports) regardless of what the code called them. So you'll see lots of `impl core::...` when the code really wrote `impl std::...` because `std` re-exports lots of things from `core`.
    * Probably not possible to fix this without parsing the source code.
* It would be nice to show crate versions, but versions are a Cargo thing, not a rustdoc thing, and so it isn't present in the JSON data anywhere.
* Related, it's not currenlty possible to show crate types; having a binary and lib crate with the same name won't work.

# Enhancements
* implement some form of live search, where you can start typing and rsbrowse selects the thing
    * initially, within the current pane would be a nice start
    * eventually, within the current crate is probably good enough
    * globally is probably a bad idea
    * let users hit F3 or something to continue to the next match
* add child items for generic parameters. Things like Arc<`Foo`> should have `Foo`'s children visible.
