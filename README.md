## convert-moves

Converts the .sml files left in /Library/Application&nbsp;Support/Suunto/Moveslink2/
to GPX files suitable for uploading to Strava.

## retrieve-moves

A little web scraper that can extract GPX files from Movescount (the web
site used by Suunto)

### Usage

Install [geckodriver](https://github.com/mozilla/geckodriver) and run it.
Then you have these options:

```
USAGE:
    retrieve-moves [FLAGS] [OPTIONS]

FLAGS:
    -e, --export     
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -m, --month <month>     [default: 11]
    -y, --year <year>       [default: 2013]
```

### Caveat Emptor
I wrote this for myself, to grab GPX files of all my moves, since Suunto
is transitioning away from Movescount.  The defaults are for the first month
and year I started using Movescount.  They're unlikely to be useful to anyone
other than myself.

Although I've successfully used this to get GPX files of all 2,200+ of
my moves, I've had to hand-hold the app and run it a few times to do
so.  I've changed some of the sleep durations between passes.  There
really shouldn't be any "long" sleeps, but there are due to my
laziness.  For example, when a GPX file is downloaded,
retrieve-moves should just wait for the file to be created and to
end with &lt;/html> instead of sleeping some number of seconds.

Additionally, this code is slow since it uses glob each time it's
checking to see if there are the right number of moves for a given
date.  It would be much quicker to simply glob all the files once and
then search through the pre-globbed files.  I may add that feature,
since I'll probably be running this app once a day until Movescount is
no longer present (or they do something to prevent this app from
working).  On the other hand, there's no way I'll be using this app
for five years, so if nobody else is using this app, it's probably not
[worth the time](https://xkcd.com/1205/). So, if you're using this
app, let me know.

I've decided to make a few of my toy projects publicly available, in
part because doing so makes me nervous and I like to get out of my
comfort zone now and then.

## Public Domain

I have released retrieve-moves into the public domain, per the
[UNLICENSE](UNLICENSE).
