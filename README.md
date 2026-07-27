FST based ad blocking DNS forwarder

### why?
dnsmasq handles large domain list rather poorly,
costs around 4 times the memory (of the textual list).
for example, current oisd big is close to 7M, after loading it,
dnsmasq RSS went up around 30MB.

also it _seems_ the lookup is not optimized, according to gemini,
I didn't really read the code.

well, to be fair, dnsmasq was never designed for this,
30MB is not much in real world, and dnsmasq still handles the list
at sub ms response time.

on the other hand, FST (finite state transducer) is a fantastic
data structure for this kind of usage, achieves compression ratio
at around 0.6 for oisd big, and it can be memory mapped
(not implemented in FSTDNS, yet).
it could run on very memory constrained home routers.

yeah I know it's a massive overkill, but, just to scratch an itch.

### other functions
* alternative upstream server for matching domains
* block queries by QTYPE

### missing features
to replace dnsmasq as the default forwarder
* cache
	* cache hit is not that high anyway
		* maybe consumer devices do their own cache now
	* maybe run it in front of dnsmasq
	* no plan to add it as of now

### config
it takes one parameter: the path to the conf file,
or `-` to read from stdin.
default to `conf` if not specified.
list of configurations:
* `listen`, default to `127.0.0.1:1053`
* `upstream`, default upstream when no rules match
* `hosts <path> <domain>`
	* `<domain>` is used to expand hosts
		* similar to dnsmasq `--expand-hosts`
	* no default when not specified
* `<condition> <action>` rules
	* conditions:
		* `qtype <qtype>`
			* takes numeric value, like 64 for svcb, 65 for https
		* `domain <domain>`
			* `example.com` matches `example.com` AND `sub.example.com`, but not `subexample.com`
			* `-` means unqualified names only (name without any dots)
		* `domain-list <file_name>[:<prefix>]`
			* similar to `domain`, each line represents a domain
			* lines started with `#` are considered comment
			* expect each domain to be prefixed with `<prefix>`,
			when specified
	* actions: `nxdomain`, `notimp`, `refused`, `upstream ...`
		* for upstream, there is also a special name `default`

examples:
```sh
listen 0.0.0.0:53
upstream 1.1.1.1 1.0.0.1
hosts /etc/hosts lan
# to resolve dhcp names if you have a dnsmasq doing that
domain - upstream 192.168.0.1
domain lan upstream 192.168.0.1
# use alternative upstream for domains listed in that file
domain-list /etc/list upstream 8.8.8.8 8.8.4.4
# block these domains
# `:*.` handles oisd _domainswild_ list intended for FreshTomato
domain-list /etc/oisd:*. nxdomain
# block query type https
qtype 65 notimp
```
due to the weak parser, strange file names will not work.

### links
* https://github.com/BurntSushi/fst