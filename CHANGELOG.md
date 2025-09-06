# Changelog

## 3.0.0 - Sep 05, 2025
- dupes: totally rewritten search algorithm with similarity detection combining fuzzy string matching (Levenshtein, Sørensen-Dice) and rare-token scoring, normalization, parallel processing, and advanced text processing
- dupes: sampling uses a three-point strategy (beginning, middle, end)
- dupes: `--sample` is now measured in KB instead of B
- list: auto enable "Show full file paths" option when multiple root dirs are given
- list: use natural sorting for displaying entries
- join: display how clashes were resolved, the target directory, and whether it will move or copy files there
- rebuild: include support for comments in collections after the sequence number, so you can add notes to your files
- rebuild: uses new pattern format `name~sequence[comment]`, with support for migrating old collections to the new one
- rebuild: do not fix gaps anymore when partial mode is enabled, which was causing unexpected renames when some files were moved; now only full mode will fix gaps
- rename: display better clashes and how they are (or not) resolved
- global: new `path_in` and `path_ex` fetch options for including and excluding full paths (and letting `dir_in` and `dir_ex` options for current dirs)
- global: use natural sorting in `--show` option for displaying entries
- global: fix `-i` failing to look for '…$' regexes, which required to remove the file extension first
- new "recipe type" options in naming rules which are advanced transformations with ready to use regexes, starting with `throw` prefixes to the end as suffixes
- new support for adding on-demand separators `{S}` in naming rules regexes to match `-`, ` ` (space), `.`, and `,` (`_` is not included as it might be part of names)
- new partial enclosing brackets support in naming rules' before and after options

## 2.0.0 - Mar 24, 2025:
- global support for **colors**!
- list: command is greatly improved with support for listing directories, including their number of files and full sizes
- global: new precise recursion feature
- global: new `--view` option
- global: input paths can now be relative.

## 1.4.0 - Feb 28, 2025:
- new `probe` command
- rebuild: new `--case` option to keep original case
- rename: included support for handling clashes by inserting sequences in the filenames

## 1.3.1 - Feb 04, 2025:
- rebuild: fix full mode, which wouldn't reset sequences

## 1.3.0 - Jan 31, 2025:
- list: smarter list command, which hides full paths by default (with a flag for showing them if needed) and uses by default descending order for size and ascending for name and path (with a flag to reverse it if needed)
- join: change no_remove flag to parents (n -> p) and some clash options
- rebuild: change simple_match flag to simple and fix full mode, which was not resetting sequences
- global: general polishing

## 1.2.1 - Nov 19, 2024:
- global: upgrade regex dependency, so deps badge won't show "maybe insecure"

## 1.2.0 - Nov 19, 2024:
- rebuild: much improved partial mode which can alter groups of filenames while preserving sequences, and even detect and fix gaps in sequences caused by deleted files

## 1.1.0 - Oct 10, 2024:
- join: support not empty target folders and resolve clashes accordingly
- join: fix join by copy still moving files
- global: include support for aliases in several enum CLI arguments

## 1.0.0 - Oct 09, 2024:
- rebuild: new partial mode, new replace feature, auto-enable partial mode in case not all directories are available
- global: major overhaul

## 0.18.0 - Aug 27, 2024:
- rebuild: new force implementation that is easier to use with improved memory usage

## 0.17.1 - Aug 15, 2024:
- global: fix `--shallow` option

## 0.17.0 - Aug 05, 2024:
- join: new clash resolve option
- global: dedup input directories
- global: support for selecting only files by filtering extensions

## 0.16.0 - Ago 01, 2024:
- new `join` command
- rename: include full directory support
- global: scan with directory support
- global: new magic filter options
- global: new filter options

## 0.15.0 - Jul 18, 2024:
- rename: nicer command output by parent directory
- global: new threaded yes/no prompt that can be aborted with CTRL-C

## 0.14.0 - Jul 11, 2024:
- rename: disallow by default changes in directories where clashes are detected, including new `--clashes` option to allow them

## 0.13.0 - Jul 10, 2024:
- rename: new replace feature
- dupes: remove case sensitivity option
- global: make strip rules also remove `.` and `_`
- global: `--include` and `--exclude` options do not check file extensions

## 0.12.0 - Jul 09, 2024:
- global: new `--dir-in` and `--dir-out` options

## 0.11.0 - Jul 08, 2024:
- new `rename` command
- rebuild, rename: improve strip exact rules

## 0.10.0 - Jul 02, 2024:
- global: new `--exclude` option

## 0.9.0 - Jul 01, 2024:
- global: support for CTRL-C

## 0.8.0 - Jun 30, 2024:
- new `list` command

## 0.7.1 - Jun 28, 2024:
- rebuild: fix smart detect not grouping some files
- global: `--include` is now case-insensitive
- global: strip rules remove hyphens too

## 0.7.0 - Jun 27, 2024:
- rebuild: new `--force`, new interactive mode, new `--yes`, auto fix rename errors, smaller memory consumption
- dupes: improved performance
- global: new `--include` option

## 0.6.0 - Jun 24, 2024:
- new `rebuild` command
- global: general polishing overall

## 0.5.0 - Jun 20, 2024:
- dupes: ignores repetition systems
- global: support for shallow scan, verbose mode

## 0.4.0 - Jun 17, 2024:
- global: new subcommands structure
- new `dupes` command, with support for matching case and changing sample size

## 0.3.0 - Nov 07, 2023:
- include support for dedup by both size and name

## 0.2.2 - Jun 04, 2022:
- use 2KB sample size

## 0.2.1 - Jun 04, 2022:
- improve error handling

## 0.2.0 - Jun 01, 2022:
- publish as `refine`, use split crate `human-repr`

## 0.1.1 - May 27, 2022:
- samples the center of the files, which seems to fix false positives

## 0.1.0 - May 25, 2022:
- first release, detects duplicated files, simple sampling strategy (1KB from the start of the files)
