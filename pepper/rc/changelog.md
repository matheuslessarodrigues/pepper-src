# changelog

# 0.28.0
- added the concept of breakpoints for plugins to use
- added bindings starting with `x` that interact with breakpoints
- changed theme color name from `background` to `normal_background`
- changed theme color name from `active_line_background` to `active_background`
- added theme color `breakpoint_background`
- changed smart search patterns: if your search pattern contains a `%` character, it will perform a pattern search instead of a fixed string search (it's still possible to force a fixed string search by prefixing it with either `f/` or `F/`)
- changed `find_path_and_position_at` to also break on `"` and `'`
- added remedybg plugin (under the `plugin-remedybg` folder)
- added css syntax
- changed bracket objects to invert bracket positions if invoked with the closing bracket. that is, `a)` will now select text between `)` and `(` instead of `(` and `)`.

# 0.27.0
- added `set-env` command to change the editor's environment variables
- fix `@arg(*)` expanding into no arguments if it's empty
- fix `save` command alias `s` not taking arguments as it should
- changed `cd` binding (delete all cursors except the main cursor) to `CD`
- added new `cd` binding that only deletes the main cursor
- added lsp configuration examples
- fix `gf` (and `GF`) that could open a duplicate of an already opened buffer if trying to open the same path but absolute
- fix `reopen-all` would fail if there was a scratch buffer with a path that does not exist
- changed `spawn` command to use a piped stdout in order to detect when the process exits
- changed `cursor-<anchor/position>-<column/line>` expansions to be one based (instead of zero based) for easier interoperability with other softwares

## 0.26.1
- improved `find_path_and_position_at` to account for paths followed by `:`
- unix: fix not being able to spawn server if the application was installed to a folder in path

## 0.26.0
- removed escaping expansion from `{...}` string blocks
- unix now uses `posix_spawn` instead of `fork` to spawn a server for better reliability and to remove the need to use `libc::daemon` which is deprecated on macos
- fixed bug on windows that prevented the server from spawning when opening files using `--` cli positional args

## 0.25.0
- new variable expansion mechanism when evaluating commands
- changed string syntax for commands
- command strings now support some escapings
- command aliases that start with `-` won't show up in auto completions
- merged `default_commands.pepper` with `default_bindings.pepper` into `default_configs.pepper`
- merged all `map-<mode>` commands into a single `map` command whose first parameter is the mode to map keys to
- merged all `syntax-<token-kind>` commands into the `syntax` command which can take the first parameter the token kind for the defined pattern
- insert processes now correctly adjust their insert positions on buffer insertions and deletions
- added `set-register` command
- changed `open` command parameters order, now buffer properties come before the `path` parameter
- removed `alias` command since it's now possible to replicate its behavior by creating a new command that calls the aliased command and use the `@arg()` expansion
- removed `find-file` and `find-command` commands as they're now implementable using other builtin commands (see `default_configs.pepper` for an example)
- removed the old 255 cursor count limit
- exiting search mode will fully restore the previous cursor state
- it's now possible to use use the search mode to expand selections
- included default config files to help pages
- fix wrong error message when parsing color values
- fix buffer would not read from file when opened with `saving-disabled`
- lsp plugin correctly handle completion responses which only fill the `changes` field
- added `pepper-` prefix to windows session named pipe paths

## 0.24.0
- handle buffer paths beginning with `./` (on `Buffer::set_path` and `Buffer::find_with_path`)
- command `$` is now `!` and what was `!` is now removed; that is, there's no longer a 'only insert from command output', just 'replace with command output' (`|` command) and if the selection is empty, it behaves as if it was the old `!`

## 0.23.3
- fix failing lsp protocol test that should only run on windows
- force redeploy on github actions

## 0.23.2
- fix URI parsing on windows

## 0.23.1
- fix crash after pc wakeup on linux (possibly on bsd and mac as well)
- fix server occasionally dropping writes to client on linux

## 0.23.0
- changed default clipboard linux interface to `xclip` instead of `xsel`
- fix crash when `lsp-references` would not load the context buffer
- handle `<c-i>` (insert at end of line) by instead mapping it to tab on unix
- fix some lsp operations not working on unix due to poor path handling

## 0.22.0
- added quit instruction to the start screen
- added '%t' to patterns to match a tab ('\t')
- fix bad handling of BSD's resize signal on kqueue

## 0.21.0
- prevent deadlocks by writing asynchronously to clients from server
- fix possible crash when listing lsp item references when there's a usage near the end of the buffer
- added instructions on how to package the web version of the editor
- added error to `lsp-stop` and `lsp-stop-all` when there is no lsp server running

## 0.20.0
- use builtin word database for completions when plugins provide no suggestions
- prevent closing all clients when the terminal from which the server was spawned is closed
- fix debugging crash when sometimes dropping a client connection

## 0.19.3
- added changelog! you can access it through `:help changelog<enter>`
- added error number to unix platform panics
- fix event loop on bsd
- fix idle events not triggering on unix
- fix buffer history undo crash when you undo after a "insert, delete then insert" single action
- fix messy multiple autocomplete on the same line
- fix crash on macos since there kqueue can't poll /dev/tty

## 0.19.2 and older
There was no official changelog before.
However, up to this point, we were implementing all features related to the editor's vision.
Then fixing bugs and stabilizing the code base.
