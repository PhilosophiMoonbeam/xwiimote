#
# WiiLand - makefile
# Written by David Herrmann, 2011-2013
# Dedicated to the Public Domain
#

#
# Library Version Numbers
#

LIBXWIIMOTE_CURRENT = 2
LIBXWIIMOTE_REVISION = 0
LIBXWIIMOTE_AGE = 0

#
# miscellaneous
#

ACLOCAL_AMFLAGS = -I m4 ${ACLOCAL_FLAGS}
AM_MAKEFLAGS = --no-print-directory
AUTOMAKE_OPTIONS = color-tests
AM_DISTCHECK_CONFIGURE_FLAGS = --enable-debug

SUBDIRS = .

.DELETE_ON_ERROR:

#
# Distribution includes
#

EXTRA_DIST = \
	README \
	COPYING \
	DEV \
	LICENSE \
	res/50-xorg-fix-wiiland.conf \
	res/70-udev-wiiland.rules \
	tests/xwii_stubs.c \
	tests/wiilandd-smoke.sh \
	tests/wiilandd-install-smoke.sh \
	tools/wiilandd-hardware-report.sh \
	.github/workflows/ci.yml \
	doc/WIILAND \
	libxwiimote.sym
EXTRA_DIST += res/wiilandd.service.in


#
# Build targets
#

lib_LTLIBRARIES = libxwiimote.la
bin_PROGRAMS = xwiishow wiilandd
noinst_PROGRAMS = xwiidump
include_HEADERS = lib/xwiimote.h
man_MANS = \
	doc/wiiland.7 \
	doc/libxwiimote.7 \
	doc/xwiishow.1 \
	doc/wiilandd.1
EXTRA_DIST += $(man_MANS)

pkgconfigdir = $(libdir)/pkgconfig
pkgconfig_DATA = libxwiimote.pc
EXTRA_DIST += \
	$(pkgconfig_DATA) \
	libxwiimote.pc.in

udevrulesdir = $(libdir)/udev/rules.d
dist_udevrules_DATA = \
	res/70-wiiland-uinput.rules

systemduserdir = $(prefix)/lib/systemd/user
nodist_systemduser_DATA = \
	res/wiilandd.service

wiilandexamplesdir = $(docdir)/examples
dist_wiilandexamples_DATA = \
	res/wiilandd.conf

res/wiilandd.service: $(srcdir)/res/wiilandd.service.in Makefile
	$(AM_V_at)$(MKDIR_P) res
	$(AM_V_GEN)sed -e 's|@bindir[@]|$(bindir)|g' \
		$(srcdir)/res/wiilandd.service.in >$@

#
# Default c/l-flags
#

AM_CFLAGS = \
	-Wall \
	-pipe \
	-fno-common \
	-ffast-math \
	-fdiagnostics-show-option \
	-fno-strict-aliasing \
	-fvisibility=hidden \
	-ffunction-sections \
	-fdata-sections \
	-fstack-protector
AM_CPPFLAGS = \
	-include $(top_builddir)/config.h \
	-I $(srcdir)/lib \
	-D'XWII__EXPORT=__attribute__((visibility("default")))'
AM_LDFLAGS = \
	-Wall \
	-Wl,--as-needed \
	-Wl,--gc-sections \
	-Wl,-z,relro \
	-Wl,-z,now

if DEBUG
AM_CFLAGS += -O0 -g
else
AM_CFLAGS += -O2
endif

#
# libxwiimote
#

libxwiimote_la_SOURCES = \
	lib/xwiimote.h \
	lib/core.c \
	lib/monitor.c

EXTRA_libxwiimote_la_DEPENDENCIES = ${top_srcdir}/libxwiimote.sym

libxwiimote_la_CPPFLAGS = \
	$(AM_CPPFLAGS) \
	$(UDEV_CFLAGS)
libxwiimote_la_LIBADD = \
	$(AM_LIBADD) \
	$(UDEV_LIBS)
libxwiimote_la_LDFLAGS = \
	$(AM_LDFLAGS) \
	-version-info $(LIBXWIIMOTE_CURRENT):$(LIBXWIIMOTE_REVISION):$(LIBXWIIMOTE_AGE) \
	-Wl,--version-script=$(top_srcdir)/libxwiimote.sym

#
# xwiishow
#

xwiishow_SOURCES = \
	tools/xwiishow.c
xwiishow_CPPFLAGS = \
	$(AM_CPPFLAGS) \
	$(NCURSES_CFLAGS)
xwiishow_LDADD = \
	$(NCURSES_LIBS) \
	libxwiimote.la \
	-lm
xwiishow_LDFLAGS = \
	$(AM_LDFLAGS)

#
# wiilandd
#

wiilandd_SOURCES = \
	tools/wiilandd.c
wiilandd_CPPFLAGS = \
	$(AM_CPPFLAGS)
wiilandd_LDADD = \
	libxwiimote.la
wiilandd_LDFLAGS = \
	$(AM_LDFLAGS)

check-local: wiilandd
	$(AM_V_at)./wiilandd --self-test
	$(AM_V_at)$(SHELL) $(top_srcdir)/tests/wiilandd-smoke.sh

#
# xwiidump
#

xwiidump_SOURCES = \
	tools/xwiidump.c
xwiidump_CPPFLAGS = \
	$(AM_CPPFLAGS)
xwiidump_LDFLAGS = \
	$(AM_LDFLAGS)

#
# doxygen
#

if HAVE_DOXYGEN

doc: doc/stamp-doxygen
clean-doc: clean-doxygen
all-local:: doc
clean-local:: clean-doc

doc/stamp-doxygen: $(top_srcdir)/lib/xwiimote.h
	$(AM_V_GEN)$(DOXYGEN) doc/Doxyfile
	$(AM_V_at)touch $@

clean-doxygen:
	$(AM_V_at)rm -vrf doc/html doc/stamp-doxygen

install-data-local:: doc
	$(MKDIR_P) $(DESTDIR)$(htmldir)
	$(INSTALL_DATA) doc/html/* $(DESTDIR)$(htmldir)

uninstall-local::
	rm -rf $(DESTDIR)$(htmldir)

endif # HAVE_DOXYGEN
