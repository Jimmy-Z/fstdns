FST based ad blocking DNS forwarder

# CAUTION, WIP, this document is a plan, does NOT represent what's available

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
	* no plan to support it as of now
		* maybe run it in front of dnsmasq

### config
it takes one parameter: the path to the conf file,
or `-` to read from stdin.
default to `conf` if not specified.
list of configurations:
* `listen`, default to `0.0.0.0:53`
* `default`, default upstream when no rules match
* `resolv-conf <path>`, get upstream from resolv conf
* `hosts <path> [domain]`
	* `domain` is used to expand hosts
		* like `a` in hosts will also expand to `a.lan`
			* there's no check, if hosts has `a.lan`,
			it will be expanded to `a.lan.lan`
	* no default when not specified
* `<condition> <action>` rules
	* conditions:
		* `domain <domain>`
			* `foo.bar` matches `foo.bar` AND `sub.foo.bar`, but not `subfoo.bar`
		* `domain-list <file_name>[:prefix]`
			* similar to `domain`, each line represents a domain
			* lines started with `#` are considered comment
			* expect each domain to be prefixed with `<prefix>`,
			when specified
		* `unqualified` for name without any dots
		* `qtype <qtype>` dns query type
			* a few names are supported like AAAA, SVCB, HTTPS
			* or decimal number
		* `exact <name> <qtype>`
	* actions:
		* `NXDOMAIN`
		* `NOTIMP`
		* `REFUSED`
		* `alt ...`
		* `default`
		* `rewrite <ttl> <rdata>`
			* (planned)
			* only makes sense with exact rule

example:
```sh
# use unprivileged port for debugging
listen 127.0.0.1:1053
#
default 1.1.1.1 1.0.0.1
#
hosts /etc/hosts lan
# to resolve dhcp names if you have dnsmasq doing that
# or nxdomain if you don't have it, to prevent leaking to upstream
unqualified alt 192.168.0.1
domain lan alt 192.168.0.1
# use alternative upstream for domains listed in that file
domain-list /etc/list alt 8.8.8.8 8.8.4.4
# block these domains
# `:*.` handles oisd _domainswild_ list intended for FreshTomato
domain-list /etc/oisd:*. nxdomain
# 
qtype https notimp
```
due to the weak parser, strange file names (contains space or colon) will not work.

### internals
* priority order, for example,
even with `unqualified nxdomain`, hosts records will still be served.
	* exact rules
		* internally hosts records are exact rewrite rules (a, aaaa, ptr)
	* rule for unqualified names
	* query type rule
	* domain rules
		* longest match wins
	* default upstream
* CHAOS diagnostics
	* `drill example.com txt ch`
		* test domain rules, since this skips other checks,
		it might not reflect actual behavior

### links
* https://github.com/BurntSushi/fst