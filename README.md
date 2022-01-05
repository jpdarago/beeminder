# Beeminder

Command line tool for [Beeminder REST API](https://api.beeminder.com) in
Rust.

# Status

Experimental, DO NOT USE FOR ANYTHING IMPORTANT. There are better
alternatives (e.g. https://github.com/lydgate/bmndr).

This is more of a learning exercise of Rust than anything else.

# Supported functionality

  - User endpoint.
  - Retrieving all goals for a user.
  - Retrieving a subset of the information for a goal.
  - Retrieving a subset of the information for a datapoint.
  - Creating new datapoints for a goal.
  - Adding datapoints for a goal.

# TODOs

  - Error handling for Beeminder errors.
  - Error handling of broken inputs.
  - More commands.

# Put command format

The `put` command allows adding several datapoints at the same time
using standard input.

The text format is slightly different than the one Beeminder uses, for
easier parsing and ease of use with `date` command.

The format has one data point per line formatted like this:

    <date as %Y-%m-%d %H:%M:%S> <value as floating point> '<optional comment>'

The comment is optional and can only be surrounded by single quotes.

Example:

    2021-12-04 12:00:00 124 'foo bar baz'
    2021-12-05 15:00:00 124.2

The parser is a simple regex, you can test your inputs
[here](https://regex101.com/r/46uRAz/1).

# Authentication

The tool requires the username and [API authentication token](https://api.beeminder.com/#auth).

The username and API token can be provided with the following options, in descending order of
preference.

For the token

- CLI flag `--auth_token`.
- Environment variable `BEEMINDER_AUTH_TOKEN`.
- TOML Config file `auth_token` field. The config file is located in `.config/beeminder/beeminder.toml` in Ubuntu.

For the username

- CLI flag `--username`.
- Environment variable `BEEMINDER_USERNAME`.
- TOML Config file `username`. The config file is located in `.config/beeminder/beeminder.toml` in Ubuntu.

Example contents of `.config/beeminder/beeminder.toml`:

```toml
auth_token = "thetoken"
username = "foobar"
```
