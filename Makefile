PREFIX ?= /usr/local
BINDIR = $(PREFIX)/bin
MANDIR = $(PREFIX)/share/man/man1

all: build

build:
	cargo build --release

install: build
	install -d $(DESTDIR)$(BINDIR)
	install -m 755 target/release/xdeskie $(DESTDIR)$(BINDIR)/xdeskie
	install -d $(DESTDIR)$(MANDIR)
	install -m 644 xdeskie.1 $(DESTDIR)$(MANDIR)/xdeskie.1
	gzip -f $(DESTDIR)$(MANDIR)/xdeskie.1

uninstall:
	rm -f $(DESTDIR)$(BINDIR)/xdeskie
	rm -f $(DESTDIR)$(MANDIR)/xdeskie.1.gz

clean:
	cargo clean

.PHONY: all build install uninstall clean
