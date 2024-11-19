# refine

[![Crates.io](https://img.shields.io/crates/v/refine.svg)](https://crates.io/crates/refine)
[![dependency status](https://deps.rs/repo/github/rsalmei/refine/status.svg)](https://deps.rs/repo/github/rsalmei/refine)
![Crates.io](https://img.shields.io/crates/d/refine)
![GitHub Sponsors](https://img.shields.io/github/sponsors/rsalmei)

### Refine your file collection using Rust!

## What it does

This tool will help you manage and organize your collections of files like no other! It will help you find duplicated files, rename filenames and directories with advanced regex rules, strip and replace parts of their names, join them together in a single directory, filter and extract copies of files and directories, apply sequence numbers to equivalent names, and even group and completely rebuild their names to organize them however you want, and make everything refined and easier to find.

I've made this tool to be the fastest and easiest way to organize file collections. I use it a lot, and I hope it can help you too. It will scan several given paths at once and analyze all files and directories as a whole, performing some advanced operations on them. Enjoy!

It is blazingly fast, of course, like all Rust 🦀 software!

The name comes from "_refine your [photos | images | videos | movies | porn | music | etc.] collection_"!

## How to use it

Install `refine` with:

```
cargo install refine
```

And that's it, you're ready to go! You can now call it anywhere.

## What's new

### New in 1.2

Here is a much improved partial mode in Rebuild command, which can alter groups of filenames while preserving sequences, and even detect and fix gaps in sequences caused by deleted files.

### New in 1.1

Revamped join command!
It now supports non-empty target folders, and will resolve clashes accordingly.

Also, several enum CLI arguments now support aliases, and I've fixed join command still moving files even when copy was requested.

### New in 1.0

Yes, it is time.
After a complete overhaul of the code, it's time to release 1.0!
<br>It's an accomplishment I'm proud of, which took over 70 commits and a month's work, resulting in most of the code being rewritten.
It is more mature, stable, and well-structured now.

The major motivation for this version is the rebuild Partial mode! We can now rebuild collections even when some paths are not available! This means that files not affected by the specified naming rules will stay the same, keeping their sequence numbers, while new files are appended after the highest sequence found. It is handy for collections on external drives or cloud storage which are not always connected, allowing you to, even on the go, rebuild new files without messing up previous ones.

And this also includes:

- rebuild: new `--replace` option to replace all occurrences of some string or regex in the filenames with another one.
- new internal CLI options handling, which enables commands to modify them prior to their execution.
    - the new rebuild partial mode is auto-enabled in case not all paths are currently available.

<details><summary>(previous)</summary>

### New in 0.18

- rebuild: new force implementation that is easier to use
    - it conflicts with any other options so must be used alone
    - now it just overwrites filenames without exceptions → best used with `-i` or on already organized collections
    - improved memory usage

### New in 0.17

- join: new clash resolve option
    - by default, no changes are allowed in directories where clashes are detected
    - all directories with clashes are listed, showing exactly which files are in them

### New in 0.16

- complete overhaul of the scan system, allowing directories to be extracted alongside files
- new `join` command, already with directory support
- new magic `-i` and `-x` options that filter both files and directories
- new filter options for files, directories, and extensions
- rename: include full directory support

### New in 0.15

- nicer rename command output by parent directory
- new threaded yes/no prompt that can be aborted with CTRL-C

### New in 0.14

- rename: disallow by default changes in directories where clashes are detected
    - new `--clashes` option to allow them

### New in 0.13

- rename: new replace feature, finally!
- global: make strip rules also remove `.` and `_`, in addition to `-` and spaces
- global: include and exclude options do not check extensions
- dupes: remove case option, so everything is case-insensitive now

### New in 0.12

- global: new `--dir-in` and `--dir-out` options.

### New in 0.11

- new `rename` command
- rebuild, rename: improve strip exact, not removing more spaces than needed

### New in 0.10

- global: new `--exclude` option to exclude files

### New in 0.9

- new support for Ctrl-C, to abort all operations and gracefully exit the program at any time.
    - all commands will stop collecting files when Ctrl-C is pressed
    - both `dupes` and `list` command will show partial results
    - the `rebuild` command will just exit, as it needs all the files to run

### New in 0.8

- new "list" command

### New in 0.7

- global: new `--include` option to filter input files
- rebuild: new `--force` option to easily rename new files
- rebuild: new interactive mode by default, making `--dry_run` obsolete (removed), with new `--yes` option to bypass it (good for automation)
- rebuild: auto fix renaming errors
- dupes: faster performance by ignoring groups with one file (thus avoiding loading samples)
- rebuild: smaller memory consumption by caching file extensions

</details>

## Commands

All commands will:

1. recursively scan all the given paths (excluding hidden .folders)
    - can optionally perform only a shallow scan
    - can optionally filter files based on two regexes (`--include` and `--exclude`)
    - can optionally filter directories based on two regexes (`--dir-in` and `--dir-ex`)
2. load the metadata the command requires to run (e.g., file size, creation date, etc.) for each file
3. execute the command and print the results

<details><summary>refine --help</summary>

```
Refine your file collection using Rust!

Usage: refine [OPTIONS] [PATHS]... <COMMAND>

Commands:
  dupes    Find possibly duplicated files by both size and filename
  join     Join all files into the same directory
  list     List files from the given paths
  rebuild  Rebuild the filenames of media collections intelligently
  rename   Rename files in batch, according to the given rules
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version

Global:
  -i, --include <REGEX>  Include only these files and directories; checked without extension
  -x, --exclude <REGEX>  Exclude these files and directories; checked without extension
  -I, --dir-in <REGEX>   Include only these directories
  -X, --dir-ex <REGEX>   Exclude these directories
      --file-in <REGEX>  Include only these files; checked without extension
      --file-ex <REGEX>  Exclude these files; checked without extension
      --ext-in <REGEX>   Include only these extensions
      --ext-ex <REGEX>   Exclude these extensions
  -w, --shallow          Do not recurse into subdirectories
  [PATHS]...         Paths to scan

For more information, see https://github.com/rsalmei/refine
```

</details>

## The `dupes` command

The `dupes` command will analyze and report the possibly duplicated files, either by size or name. It will even load a sample from each file, to guarantee they are indeed duplicated. It is a small sample by default but can help reduce false positives a lot, and you can increase it if you want.

1. group all the files by size
2. for each group with the exact same value, load a sample of its files
3. compare the samples with each other and find possible duplicates
4. group all the files by words in their names
    - the word extractor ignores sequence numbers like file-1, file copy, file-3 copy 2, etc.
5. run 2. and 3. again, and print the results

<details><summary>refine dupes --help</summary>

```
Find possibly duplicated files by both size and filename

Usage: refine dupes [OPTIONS] [PATHS]...

Options:
  -s, --sample <BYTES>  Sample size in bytes (0 to disable) [default: 2048]
  -h, --help            Print help
```

</details>

Example:

```
❯ refine dupes ~/Downloads /Volumes/External --sample 20480
```

## The `join` command

The `join` command will let you join all the files and directories in the given paths into the same directory. You can filter files however you like, and choose how they will be joined, either moving or copying them. It will even remove the empty parent directories after joining!

> Note: any deletions are only performed after files and directories have been successfully moved/copied. So, in case any errors occur, the files and directories partially moved/copied will be found in the target directory, so you should manually delete them before trying again.

1. detect clashes, files with the same name in different directories, and apply a sequential number
2. detect already in-place files
3. print the resulting changes to the filenames and directories, and ask for confirmation
4. if the user confirms, apply the changes
5. remove any empty parent directories

<details><summary>refine join --help</summary>

```
Join all files into the same directory

Usage: refine join [OPTIONS] [PATHS]...

Options:
  -t, --to <PATH>            The target directory; will be created if it doesn't exist [default: .]
  -s, --strategy <STRATEGY>  The strategy to use to join [default: move] [possible values: move, copy]
  -c, --clash <CLASH>        Specify how to resolve clashes [default: sequence] [possible values: sequence, parent, skip]
  -f, --force                Force joining already in place files and directories, i.e., in subdirectories of the target
  -n, --no-remove            Do not remove the empty parent directories after joining
  -y, --yes                  Skip the confirmation prompt, useful for automation
  -h, --help                 Print help
```

</details>

Example:

```
❯ refine join ~/media/ /Volumes/External/ -i 'proj-01' -X 'ongoing' -t /Volumes/External/proj-01
```

## The `list` command

The `list` command will gather all the files in the given paths, sort them by name, size, or path, and display them in a friendly format.

1. sort all files by either name, size, or path
    - ascending by default, or optionally descending
2. print the results

<details><summary>refine list --help</summary>

```
List files from the given paths

Usage: refine list [OPTIONS] [PATHS]...

Options:
  -b, --by <BY>  Sort by [default: name] [possible values: name, size, path]
  -d, --desc     Use descending order
  -h, --help     Print help
```

</details>

Example:

```
❯ refine list ~/Downloads /Volumes/External --by size --desc
```

## The `rebuild` command

The `rebuild` command is a great achievement, if I say so myself. It will smartly rebuild the filenames of an entire collection when it is composed by user ids or streamer names, for instance. It will do so by removing sequence numbers, stripping parts of filenames you don't want, smartly detecting misspelled names by comparing with adjacent files, sorting the detected groups deterministically by creation date, regenerating the sequence numbers, and finally renaming all the files accordingly. It's awesome to quickly find your video or music library neatly sorted automatically... And the next time you run it, it will detect new files added since the last time, and include them in the correct group! Pretty cool, huh? And don't worry, you can review all the changes before applying them.

1. if forced mode is enabled:
    1. overwrite all the filenames with the forced one
2. else if partial mode is enabled:
    1. apply naming rules to strip or replace parts of the filenames, marking modified files
    2. strip sequence numbers from changed files, and extract/store the highest sequence from unchanged files
3. otherwise:
    1. apply naming rules to strip or replace parts of the filenames
    2. strip sequence numbers from all files
4. remove spaces and underscores, and smartly detect misspelled names
5. group the resulting names, and smartly choose the most likely correct name among the group
6. sort the group content according to the files' created dates
7. regenerate the sequence numbers for each group ← Note that groups can contain files from different directories, and it will just work
    1. if partial mode is enabled, retrieve the highest sequence found in the group
8. print the resulting changes to the filenames, and ask for confirmation
9. if the user confirms, apply the changes

<details><summary>refine rebuild --help</summary>

```
Rebuild the filenames of media collections intelligently

Usage: refine rebuild [OPTIONS] [PATHS]...

Options:
  -b, --strip-before <STR|REGEX>    Strip from the start of the filename; blanks nearby are automatically removed
  -a, --strip-after <STR|REGEX>     Strip to the end of the filename; blanks nearby are automatically removed
  -e, --strip-exact <STR|REGEX>     Strip all occurrences in the filename; blanks nearby are automatically removed
  -r, --replace <STR|REGEX=STR|$N>  Replace all occurrences in the filename with another; blanks are not touched
  -s, --no-smart-detect             Disable smart detection of similar filenames (e.g. "foo bar.mp4", "FooBar.mp4" and "foo__bar.mp4")
  -f, --force <STR>                 Force to overwrite filenames (use the Global options to filter files)
  -p, --partial                     Assume not all paths are available, so only touch files actually modified by the given rules
  -y, --yes                         Skip the confirmation prompt, useful for automation
  -h, --help                        Print help
```

</details>

Example:

```
❯ refine rebuild ~/media /Volumes/External -a 720p -a Bluray -b xpto -e old
```

## The `rename` command

The `rename` command will let you batch rename files like no other tool, seriously! You can quickly strip common prefixes, suffixes, and exact parts of the filenames, as well as apply any regex replacements you want. By default, in case a filename ends up clashing with other files in the same directory, that whole directory will be disallowed to make any changes. The list of clashes will be nicely formatted and printed, so you can manually check them. And you can optionally allow changes to other files in the same directory, removing only the clashes if you find it safe.

1. apply naming rules to strip or replace parts of the filenames
2. remove all changes from the whole directory where clashes are detected
    - optionally removes only the clashes, allowing other changes
3. print the resulting changes to the filenames and directories, and ask for confirmation
4. if the user confirms, apply the changes

<details><summary>refine rename --help</summary>

```
Rename files in batch, according to the given rules

Usage: refine rename [OPTIONS] [PATHS]...

Options:
  -b, --strip-before <STR|REGEX>    Strip from the start of the filename; blanks nearby are automatically removed
  -a, --strip-after <STR|REGEX>     Strip to the end of the filename; blanks nearby are automatically removed
  -e, --strip-exact <STR|REGEX>     Strip all occurrences in the filename; blanks nearby are automatically removed
  -r, --replace <STR|REGEX=STR|$N>  Replace all occurrences in the filename with another; blanks are not touched
  -c, --clashes                     Allow changes in directories where clashes are detected
  -y, --yes                         Skip the confirmation prompt, useful for automation
  -h, --help                        Print help
```

</details>

Example:

```
❯ refine rename ~/media /Volumes/External -b "^\d+_" -r '([^\.]*?)\.=$1 '
```

## Changelog

<details><summary>(click to expand)</summary>

- 1.2.0 Nov 19, 2024: rebuild: much improved partial mode which can alter groups of filenames while preserving sequences, and even detect and fix gaps in sequences caused by deleted files.
- 1.1.0 Oct 10, 2024: join: support not empty target folders and resolve clashes accordingly; include support for aliases in several enum CLI arguments; fix join by copy still moving files.
- 1.0.0 Oct 09, 2024: major overhaul; rebuild: new partial mode, new replace feature, auto-enable partial mode in case not all paths are available.
- 0.18.0 Aug 27, 2024: rebuild: new force implementation that is easier to use with improved memory usage.
- 0.17.1 Aug 15, 2024: global: fix `--shallow` option.
- 0.17.0 Aug 05, 2024: global: dedup input paths, enables to select only files by filtering extensions; join: new clash resolve option.
- 0.16.0 Ago 01, 2024: global: scan with directory support, new `join` command, new magic filter options, new filter options; rename: include full directory support.
- 0.15.0 Jul 18, 2024: rename: nicer command output by parent directory; new threaded yes/no prompt that can be aborted with CTRL-C.
- 0.14.0 Jul 11, 2024: rename: disallow by default changes in directories where clashes are detected, including new `--clashes` option to allow them.
- 0.13.0 Jul 10, 2024: rename: new replace feature; global: make strip rules also remove `.` and `_`, `--include` and `--exclude` options do not check file extensions; dupes: remove case sensitivity option.
- 0.12.0 Jul 09, 2024: global: new `--dir-in` and `--dir-out` options.
- 0.11.0 Jul 08, 2024: global: new `rename` command; rebuild, rename: improve strip exact.
- 0.10.0 Jul 02, 2024: global: new `--exclude`.
- 0.9.0 Jul 01, 2024: global: support for CTRL-C.
- 0.8.0 Jun 30, 2024: new `list` command.
- 0.7.1 Jun 28, 2024: global: `--include` is now case-insensitive; rebuild: fix smart detect not grouping some files, strip rules remove hyphens too.
- 0.7.0 Jun 27, 2024: global: new `--include`; rebuild: new `--force`, new interactive mode, new `--yes`, auto fix rename errors, smaller memory consumption; dupes: improved performance.
- 0.6.0 Jun 24, 2024: global: new `rebuild` command, general polishing overall.
- 0.5.0 Jun 20, 2024: support for shallow scan, verbose mode; dupes: ignores repetition systems.
- 0.4.0 Jun 17, 2024: include `dupes` command, support match case and changing sample size.
- 0.3.0 Nov 07, 2023: include dedup by both size and name.
- 0.2.2 Jun 04, 2022: use 2KB sample size.
- 0.2.1 Jun 04, 2022: improve error handling.
- 0.2.0 Jun 01, 2022: publish as `refine`, use split crate `human-repr`.
- 0.1.1 May 27, 2022: samples the center of the files, which seems to fix false positives.
- 0.1.0 May 25, 2022: first release, detects duplicated files, simple sampling strategy (1KB from the start of the files).

</details>

## License

This software is licensed under the MIT License. See the LICENSE file in the top distribution
directory for the full license text.


---
Maintaining an open source project is hard and time-consuming, and I've put much ❤️ and effort into
this.

If you've appreciated my work, you can back me up with a donation! Thank you. 😊

[<img align="right" src="https://cdn.buymeacoffee.com/buttons/default-orange.png" width="217px" height="51x">](https://www.buymeacoffee.com/rsalmei)
[<img align="right" alt="Donate with PayPal button" src="https://www.paypalobjects.com/en_US/i/btn/btn_donate_LG.gif">](https://www.paypal.com/donate?business=6SWSHEB5ZNS5N&no_recurring=0&item_name=I%27m+the+author+of+alive-progress%2C+clearly+and+about-time.+Thank+you+for+appreciating+my+work%21&currency_code=USD)

---
