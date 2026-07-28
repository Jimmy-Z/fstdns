FST based ad blocking DNS forwarder

# CAUTION, WIP, this document is a plan, nothing is actually written yet

### why?
dnsmasq handles large domain list rather poorly,
costs around 4 times the memory (compared to the textual list).
for example, current oisd big is close to 7M, after loading it,
dnsmasq RSS went up around 30M.

also it _seems_ the lookup is not optimized, according to gemini,
I didn't really read the code.

well, to be fair, dnsmasq was never designed for this,
30MB is not much in real world, and dnsmasq still handles the list
at sub ms response time.

on the other hand, FST (finite state transducer) is a fantastic
data structure for this kind of workload,
achieves compression ratio at around 0.6 for oisd big,
handles prefix/suffix search naturally,
and it can be memory mapped (not implemented in FSTDNS though).
it could run on very memory constrained home routers.

yeah I know this is over-engineering, but, just to scratch an itch.

### other functions
* alternative upstream server for matching domains
* block queries by QTYPE

### missing features
* cache
	* cache hit is not that high anyway
		* maybe consumer devices do their own cache now
	* no plan to support it as of now
		* maybe run it in front of dnsmasq

### config
takes one parameter: path to a conf file,
list of configurations:
* `listen`, default to `0.0.0.0:53`
	* if specified multiple times, newer overwrites older
* `default`, default upstream when no rules match
* `resolv-conf <path>`, get upstream from resolv conf
	* if specified multiple times, newer overwrites older
	* `default` and `resolv-conf ...` overwrites each other
* `hosts <path> [domain]`
	* `domain` is used to expand hosts
		* like `a` in hosts will also expand to `a.lan`
	* they're internall handles as exact a/aaaa/ptr rewrite
* `<condition> <action>` rules
	* conditions:
		* `domain <domain>`
			* `foo.bar` matches `foo.bar` AND `sub.foo.bar`,
			but not `subfoo.bar`
		* `domain-list <file_name>[^prefix]`
			* similar to `domain`, each line represents a domain
			* lines started with `#` are considered comment
			* expect each domain to be prefixed with `<prefix>`,
			when specified
		* `unqualified` for name without any dots
		* `qtype <qtype>` dns query type
			* a few names are supported like AAAA, SVCB, HTTPS
			* or decimal number
		* `exact <qtype> <name_or_cidr>`
	* actions:
		* `NXDOMAIN`
		* `NOTIMP`
		* `REFUSED`
		* `alt ...`
		* `default`
		* `rewrite <rdata>`
			* (planned)
			* only makes sense with exact rule

examples:
```sh
# use unprivileged port for debugging
listen 127.0.0.1:1053
# set default upstream
default 1.1.1.1 1.0.0.1
# read hosts and expand with domain `lan`
hosts /etc/hosts lan
# use alternative upstream for domains listed in that file
domain-list /etc/list alt 8.8.8.8 8.8.4.4
# block these domains
# `^*.` handles oisd _domainswild_ list intended for FreshTomato
domain-list /etc/oisd^*. nxdomain
# stop lan queries from leaking to the internet/isp
domain lan nxdomain
unqualified nxdomain
# or have dnsmasq resolving dhcp names
domain lan alt 192.168.0.1
unqualified alt 192.168.0.1
# blocks query type https
qtype https notimp
# let dnsmasq resolve ptr for 192.168.0.0/24
domain 0.168.192.in-addr.arpa alt 192.168.0.1
```
due to the weak parser, strange file names (contains space or ^) will not work.

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

### tips
* to specify alternative upstream for ptr queries: `domain 0.168.192.in-addr.arpa`
* to stop lan queries from leaking to isp without having dnsmasq for local/dhcp resolve:
	```
	unqualified nxdomain
	domain lan nxdomain
	```

### links
* https://github.com/BurntSushi/fst
