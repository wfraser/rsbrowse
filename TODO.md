# Bugs
* a pane with only one item can never be expanded to the next pane
* types that impl a trait and override provided methods have both shown

# Enhancements
* format the label of functions to include the signature
    * probably will invole string munging, sticking the `.name` into the the `.value` of the def in the right spot
* handle impls of traits defined in other crates
* when there's no scrollbar, there should still be 1 column of spacing between panes
* make the color scheme less ugly
* implement some form of live search, where you can start typing and rsbrowse selects the thing
    * initially, within the current pane would be a nice start
    * eventually, within the current crate is probably good enough
    * globally is probably a bad idea
* show something when you've gotten to a leaf node
    * documentation
    * source code, using span info?
    * should this be another pane, or should it go in the .on_select popup?
        * if popup, relegate the current debug info somewhere else, it's still useful
* show visibility info (pub / pub(crate) / not pub / etc)
    * probably not possible with current RLS data
    * could maybe hack it by parsing the source code span?
