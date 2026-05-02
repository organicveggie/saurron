---
layout: page
title: Scheduling
parent: Configuration
---

<!-- prettier-ignore-start -->
# Scheduling Options
{: .no_toc }

<!-- prettier-ignore-end -->

Settings to control scheduling. The polling interval, cron, and run once options are all mutually
exclusive.

<!-- prettier-ignore-start -->
* TOC
{:toc}

<!-- prettier-ignore-end -->

## Polling interval

CLI flag
: `--interval <duration>`

Environment
: `SAURRON_POLL_INTERVAL`

TOML key
: `poll_interval`

Poll interval as a duration (e.g. `5m`, `1h`, etc). Internally, this is converted to a cron schedule (see below) relative to the start time.

## Cron schedule

CLI flag
: `--schedule <cron>`

Environment
: `SAURRON_SCHEDULE`

TOML key
: `schedule`

Poll schedule as cron expression (e.g. `* 0 4 * * * *`). The supported fields, from left to right,
include:

| Field        | Allowed values                       |
| ------------ | ------------------------------------ |
| second       | 0-59                                 |
| minute       | 0-59                                 |
| hour         | 0-23                                 |
| day of month | 1-31                                 |
| month        | 1-12 (or names)                      |
| day of week  | 0-7 (0 or 7 is Sunday, or use names) |
| year         |                                      |

A field may contain an asterisk `*`, which always stands for "first-last”.

Ranges of numbers are allowed. Ranges are two numbers separated with a hyphen. The specified range is inclusive. For example, 8-11 for an `hours` entry specifies execution at hours 8, 9, 10, and 11. The first number must be less than or equal to the second one.

Lists are allowed.  A list is a set of numbers (or ranges) separated by commas.  Examples: `1,2,5,9`, `0-4,8-12`.

Step values can be used in conjunction with ranges.  Following a range with `/<number>` specifies skips of the number's value through the range.  For example, `0-23/2` can be used in the `hours` field to specify command execution for every other (the alternative is `0,2,4,6,8,10,12,14,16,18,20,22`).  Step values are also permitted after an asterisk, so if specifying Saurron should run every two hours, you can use `*/2`.

{: .note }
Steps are evaluated only within the field they are applied to. For example `*/23` in hours field means to execute the job on the hour `0` and the hour `23` within a calendar day.

Names can also be used for the `month` and `day of week` fields. Use the first three letters of the particular day or month (case does not matter). Ranges and lists of names are allowed Examples: `mon,wed,fri`, `jan-mar`.

Year can be a specific year (e.g. `1996`), a list of years (e.g. `1999,2001`), a range of years (e.g. `1996-2001`), a step range  (e.g. `2026-2030/2` or `*/2`).

{: .warning}
Unlike crontab, this does not support the use of the `~` character for a random number in a range.

### Examples

`0 0 */2 * * * *`
: Run every 2 hours on the hour

`0 15 2 * * * *`
: Run every day at 02:15 in the morning

`0 0 2,22 * * 1,3,5 *`
: Run at 02:00 and 22:00 on Mon, Wed, and Fri

## Run once

CLI flag
: `--run-once`

Environment
: `SAURRON_RUN_ONCE`

TOML key
: `run_once`

Perform a single update cycle then exit.
