# Bugs
* ???

# Enhancements
* format the label of functions to include the signature
    * probably will invole string munging, sticking the `.name` into the the `.value` of the def in the right spot
* handle impls of traits defined in other crates
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
