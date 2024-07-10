# refine

### Refine your file collection using Rust!

## What it does

This is the tool that will help you manage your file collection! It will scan some given paths and analyze all files, performing some advanced operations on them, such as finding possibly duplicated files, batch renaming them, or even automatically grouping and rebuilding their filenames according to your rules!

It is blazingly fast and tiny, made 100% in Rust 🦀!

In the future, this tool could make much more, like for instance moving duplicated files, renaming files without rebuilding everything, perhaps supporting aliases for names, including a GUI to enable easily acting upon files, etc., hence the open `refine` (your filesystem) name...

## New in 0.13

- rebuild: new replace feature, finally!
- rebuild, rename: make strip options also remove `.` and `_`, in addition to `-` and spaces
- global: include and exclude options do not check extensions
- dupes: remove case option, so everything is case-insensitive now

<details><summary>New in 0.12</summary>

- global: new --dir-in and --dir-out options.

</details>
<details><summary>New in 0.11</summary>

- new `rename` command
- rebuild, rename: improve strip exact, not removing more spaces than needed

</details>
<details><summary>New in 0.10</summary>

- global: new --exclude option to exclude files

</details>
<details><summary>New in 0.9</summary>

- new support for Ctrl-C, to abort all operations and gracefully exit the program at any time.
    - all commands will stop collecting files when Ctrl-C is pressed
    - both `dupes` and `list` command will show partial results
    - the `rebuild` command will just exit, as it needs all the files to run

</details>
<details><summary>New in 0.8</summary>

- new "list" command

</details>
<details><summary>New in 0.7</summary>

- global: new --include option to filter input files
- rebuild: new --force option to easily rename new files
- rebuild: new interactive mode by default, making --dry_run obsolete (removed), with new --yes option to bypass it (good for automation)
- rebuild: auto fix renaming errors
- dupes: faster performance by ignoring groups with 1 file (thus avoiding loading samples)
- rebuild: smaller memory consumption by caching file extensions

</details>

## How to use it

Install `refine` with:

```
cargo install refine
```

That's it, and you can then just call it anywhere!

## Commands

All commands will:

1. recursively scan all the given paths (excluding hidden .folders)
    - can optionally perform only a shallow scan
    - can optionally filter files based on two regexes (--include and --exclude)
    - can optionally filter directories based on two regexes (--dir-in and --dir-ex)
2. load the metadata the command requires to run (e.g. file size, creation date, etc.) for each file
3. execute the command and print the results

<details><summary>refine --help</summary>

```
Refine your file collection using Rust!

Usage: refine [OPTIONS] [PATHS]... <COMMAND>

Commands:
  dupes    Find possibly duplicated files by both size and filename
  rebuild  Rebuild the filenames of media collections intelligently
  list     List files from the given paths
  rename   Rename files in batch, according to the given rules
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version

Global:
  -i, --include <REGEX>  Include only these files; checked against filename without extension, case-insensitive
  -x, --exclude <REGEX>  Exclude these files; checked against filename without extension, case-insensitive
      --dir-in <REGEX>   Include only these subdirectories; case-insensitive
      --dir-ex <REGEX>   Exclude these subdirectories; case-insensitive
      --shallow          Do not recurse into subdirectories
  [PATHS]...         Paths to scan

For more information, see https://github.com/rsalmei/refine
```

</details>

### The `dupes` command

The `dupes` command will analyze and report the possibly duplicated files, either by size or name. It will even load a sample from each file, in order to guarantee they are indeed duplicated. It is a small sample by default but can help reduce false positives a lot, and you can increase it if you want.

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

Global:
  -i, --include <REGEX>  Include only these files; checked against filename without extension, case-insensitive
  -x, --exclude <REGEX>  Exclude these files; checked against filename without extension, case-insensitive
      --dir-in <REGEX>   Include only these subdirectories; case-insensitive
      --dir-ex <REGEX>   Exclude these subdirectories; case-insensitive
      --shallow          Do not recurse into subdirectories
  [PATHS]...         Paths to scan
```

</details>

Example:

```
❯ refine dupes ~/Downloads /Volumes/External --sample 20480
```

### The `rebuild` command

The `rebuild` command is a great achievement, if I say so myself. It will smartly rebuild the filenames of an entire collection when it is composed by user ids or streamer names, for instance. It will do so by removing sequence numbers, stripping parts of filenames you don't want, smartly detecting misspelled names by comparing with adjacent files, sorting the detected groups deterministically by creation date, regenerating the sequence numbers, and finally renaming all the files accordingly. It's awesome to quickly find your video or music library neatly sorted automatically... And the next time you run it, it will detect new files added since the last time, and include them in the correct group! Pretty cool, huh? And don't worry, you can review all the changes before applying them.

1. remove any sequence numbers like file-1, file copy, file-3 copy 2, etc.
2. strip parts of the filenames, either before, after, or exactly a certain string
3. smartly remove spaces and underscores, in order to detect misspelled names
4. group the resulting names, and smartly choose the most likely correct name among the group
5. sort the group according to the file created date
6. regenerate the sequence numbers for the group <-- Note this occurs on the whole group, regardless of the directory the file currently resides in
7. print the resulting changes to the filenames, and ask for confirmation
8. if the user confirms, apply the changes to the filenames

<details><summary>refine rebuild --help</summary>

```
Rebuild the filenames of media collections intelligently

Usage: refine rebuild [OPTIONS] [PATHS]...

Options:
  -b, --strip-before <STR|REGEX>  Remove from the start of the filename to this str; blanks are automatically removed
  -a, --strip-after <STR|REGEX>   Remove from this str to the end of the filename; blanks are automatically removed
  -e, --strip-exact <STR|REGEX>   Remove all occurrences of this str in the filename; blanks are automatically removed
  -s, --no-smart-detect           Detect and fix similar filenames (e.g. "foo bar.mp4" and "foo__bar.mp4")
  -f, --force <STR>               Easily set filenames for new files. BEWARE: use only on already organized collections
  -y, --yes                       Skip the confirmation prompt, useful for automation
  -h, --help                      Print help

Global:
  -i, --include <REGEX>  Include only these files; checked against filename without extension, case-insensitive
  -x, --exclude <REGEX>  Exclude these files; checked against filename without extension, case-insensitive
      --dir-in <REGEX>   Include only these subdirectories; case-insensitive
      --dir-ex <REGEX>   Exclude these subdirectories; case-insensitive
      --shallow          Do not recurse into subdirectories
  [PATHS]...         Paths to scan
```

</details>

Example:

```
❯ refine rebuild ~/media /Volumes/External -a 720p -a Bluray -b xpto -e old
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

Global:
  -i, --include <REGEX>  Include only these files; checked against filename without extension, case-insensitive
  -x, --exclude <REGEX>  Exclude these files; checked against filename without extension, case-insensitive
      --dir-in <REGEX>   Include only these subdirectories; case-insensitive
      --dir-ex <REGEX>   Exclude these subdirectories; case-insensitive
      --shallow          Do not recurse into subdirectories
  [PATHS]...         Paths to scan
```

</details>

Example:

```
❯ refine list ~/Downloads /Volumes/External --by size --desc
```

## The `rename` command

The `rename` command will let you batch rename files like no other tool. You can quickly strip common prefixes, suffixes, and exact parts of the filenames.

1. strip parts of the filenames, either before, after, or exactly a certain string
2. print the resulting changes to the filenames, and ask for confirmation
3. if the user confirms, apply the changes to the filenames

<details><summary>refine rename --help</summary>

```
Rename files in batch, according to the given rules

Usage: refine rename [OPTIONS] [PATHS]...

Options:
  -b, --strip-before <STR|REGEX>   Remove from the start of the filename to this str; blanks are automatically removed
  -a, --strip-after <STR|REGEX>    Remove from this str to the end of the filename; blanks are automatically removed
  -e, --strip-exact <STR|REGEX>    Remove all occurrences of this str in the filename; blanks are automatically removed
  -r, --replace <{STR|REGEX}=STR>  Replace all occurrences of one str by another; applied in order and after the strip rules
  -y, --yes                        Skip the confirmation prompt, useful for automation
  -h, --help                       Print help

Global:
  -i, --include <REGEX>  Include only these files; checked against filename without extension, case-insensitive
  -x, --exclude <REGEX>  Exclude these files; checked against filename without extension, case-insensitive
      --dir-in <REGEX>   Include only these subdirectories; case-insensitive
      --dir-ex <REGEX>   Exclude these subdirectories; case-insensitive
      --shallow          Do not recurse into subdirectories
  [PATHS]...         Paths to scan
```

</details>

Example:

```
❯ refine rename ~/media /Volumes/External -b "^\d+_" -r '([^\.]*?)\.=$1 '
```

## Changelog highlights

- 0.13.0 Jul 10, 2024: rebuild: new replace feature, rebuild, rename: make strip options remove `.` and `_`, global: include and exclude options do not check extensions, dupes: remove case option.
- 0.12.0 Jul 09, 2024: global: new --dir-in and --dir-out options.
- 0.11.0 Jul 08, 2024: new `rename` command, rebuild, rename: improve strip exact.
- 0.10.0 Jul 02, 2024: global: new --exclude.
- 0.9.0 Jul 01, 2024: global: support for CTRL-C.
- 0.8.0 Jun 30, 2024: new `list` command.
- 0.7.1 Jun 28, 2024: global: --include is now case-insensitive, rebuild: fix smart detect bug not grouping some files, rebuild: strip rules remove hyphens too.
- 0.7.0 Jun 27, 2024: global: new --include, rebuild: new --force, rebuild: new interactive mode, rebuild: new --yes, rebuild: auto fix rename errors, rebuild: smaller memory consumption, dupes: improved performance.
- 0.6.0 Jun 24, 2024: new `rebuild` command, general polishing overall.
- 0.5.0 Jun 20, 2024: support for shallow scan, verbose mode, dupes cmd ignores repetition systems.
- 0.4.0 Jun 17, 2024: include `dupes` command, support match case and changing sample size.
- 0.3.0 Nov 07, 2023: include dedup by both size and name.
- 0.2.2 Jun 04, 2022: use 2KB sample size.
- 0.2.1 Jun 04, 2022: improve error handling.
- 0.2.0 Jun 01, 2022: publish, use split crate `human-repr`.
- 0.1.1 May 27, 2022: samples the center of the files, which seems to fix false positives.
- 0.1.0 May 25, 2022: first release, detects duplicated files, simple sampling strategy (1KB from
  the start of the files).

## License

This software is licensed under the MIT License. See the LICENSE file in the top distribution
directory for the full license text.


---
Maintaining an open source project is hard and time-consuming, and I've put much ❤️ and effort into
this.

If you've appreciated my work, you can back me up with a donation! Thank you 😊

[<img align="right" src="https://cdn.buymeacoffee.com/buttons/default-orange.png" width="217px" height="51x">](https://www.buymeacoffee.com/rsalmei)
[<img align="right" alt="Donate with PayPal button" src="https://www.paypalobjects.com/en_US/i/btn/btn_donate_LG.gif">](https://www.paypal.com/donate?business=6SWSHEB5ZNS5N&no_recurring=0&item_name=I%27m+the+author+of+alive-progress%2C+clearly+and+about-time.+Thank+you+for+appreciating+my+work%21&currency_code=USD)

---
