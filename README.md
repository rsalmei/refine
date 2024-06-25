# refine

### Refine your file collection using Rust!

## What it does

This is a tool that will scan any given paths, and run some command on them.

The `dupes` command will analyze and report the possibly duplicated files, both by size and name. It will even load a sample from each file, in order to guarantee they are indeed duplicated.

The new `rebuild` command is a great achievement, if I say so myself, which will smartly rebuild the filenames of your entire collection! It can strip parts of filenames, remove previous sequence numbers, smartly detect misspelled names by comparing with the other files, sort the detected file groups by creation date, and finally regenerate the sequence numbers, renaming all files accordingly...

It is blazingly fast and tiny, made 100% in Rust 🦀!

In the future, this tool could make much more, like for instance moving duplicated files, including a GUI to enable easily acting upon them, etc., hence the open name `refine`...

## How to use it

Install with:

```
cargo install refine
```

Then just call it anywhere:

```bash
❯ refine dupes ~/Downloads /Volumes/Drive ...
```

Or:

```bash
❯ refine rebuild ~/Downloads /Volumes/Drive ...
```

Send as many sources as you want.

## How it works

Command options:

```
Refine your file collection using Rust!

Usage: refine [OPTIONS] [PATHS]... <COMMAND>

Commands:
  dupes    Find possibly duplicated files by both size and filename
  rebuild  Rebuild the filenames of collections of files intelligently
  help     Print this message or the help of the given subcommand(s)

Arguments:
  [PATHS]...  Paths to scan

Options:
      --shallow  Do not recurse into subdirectories
  -h, --help     Print help
  -V, --version  Print version
```

### The `dupes` command

1. recursively detect all files in the given paths (excluding hidden .folders)
    - can optionally run only a shallow scan too.
2. sort all the files by their sizes and by their words
    - the word extractor ignores repetition systems like -1, -2, and copy, copy 2.
3. for each group with the exact same value, a sample of each file will be retrieved and compared
4. each coincidence will be listed as possible duplicates:

Command options:

```
Find possibly duplicated files by both size and filename

Usage: refine dupes [OPTIONS] [PATHS]...

Arguments:
  [PATHS]...  Paths to scan

Options:
  -s, --sample <SAMPLE>  Sample size in bytes (0 to disable) [default: 2048]
  -c, --case             Case-sensitive file name comparison
      --shallow          Do not recurse into subdirectories
  -h, --help             Print help
```

Output:

```
-- by size

132.1kB x3
/Users/you/Downloads/path/file.ext
/Users/you/Downloads/another-path/other.any
/Volumes/External/backup-path/back.001

248.6MB x2
/Users/you/Downloads/video.mp4
/Volumes/External/backup-path/video.mpg.bak

-- by name

["bin", "cache", "query"] x2
904.2kB: ./target/debug/incremental/refine-1uzt8yoeb0t1e/s-gx7knsxvbx-1oc90bk-working/query-cache.bin
904.9kB: ./target/debug/incremental/refine-1uzt8yoeb0t1e/s-gx7knwsqka-w784iw-6s3nzkfcj1wxagnjubj1pm4v6/query-cache.bin

```

And, finally, a brief receipt will be printed:

```
total files: 13512
  by size: 339 duplicates
  by name: 12 duplicates
```

### The `rebuild` command

1. strip parts of the filenames, either before or after some matches, or exact ones in the middle;
2. remove all sequence numbers they might have, like "copy 2" or "-3";
3. smartly remove spaces and underscores to detect misspelled names;
4. group the names according to the rest;
5. smartly choose the most likely correct name among the group;
6. sort the group entries by created date;
7. regenerate a unified sequence with this new order; <-- Note this occurs on the whole group,
   regardless
   of the directory the file resides!
8. renames the files to the new pattern.

Command options:

```
Rebuild the filenames of collections of files intelligently

Usage: refine rebuild [OPTIONS] [PATHS]...

Arguments:
  [PATHS]...  Paths to scan

Options:
  -b, --strip-before <STRIP_BEFORE>  Remove from the start of the filename to this str; blanks are automatically removed
  -a, --strip-after <STRIP_AFTER>    Remove from this str to the end of the filename; blanks are automatically removed
  -e, --strip-exact <STRIP_EXACT>    Remove all occurrences of this str in the filename; blanks are automatically removed
      --shallow                      Do not recurse into subdirectories
  -s, --no-smart-detect              Detects and fixes similar filenames (e.g. "foo bar.mp4" and "foo__bar.mp4")
      --dry-run                      Do not touch the filesystem, just print what would be done
  -h, --help                         Print help

```

Output:

```
/Users/you/Downloads/path/file.mp4 --> file-1.mp4
/Users/you/Downloads/path/video ok.mp4 --> video__ok-1.mp4
/Users/you/Downloads/another-path/video_ok.mp4 --> video__ok-2.mp4
/Volumes/External/backup-path/Video__OK.mp4 --> video__ok-3.mp4
/Users/you/Downloads/another-path/video not ok.mp4 --> video_not_ok-1.mp4
```

And, finally, a brief receipt will be printed:

```
total files: 21126
  changes: 1432
```

## Changelog

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
