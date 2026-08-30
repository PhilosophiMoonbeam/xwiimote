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
AM_DISTCHECK_CONFIGURE_FLAGS = \
	--enable-debug \
	--enable-xwiishow=no \
	--enable-qt-ui=no \
	--with-udev-rules-dir=no \
	--with-systemd-user-unit-dir=no \
	--with-xorg-conf-dir=no

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
	res/70-wiiland-uinput.rules \
	res/io.github.philosophimoonbeam.wiiland-config.desktop \
	res/io.github.philosophimoonbeam.wiiland.svg \
	res/wiilandd.conf \
	res/wiilandd.service.in \
	doc/wiiland.7 \
	doc/libxwiimote.7 \
	doc/xwiishow.1 \
	doc/wiilandd.1 \
	doc/wiiland-config.1 \
	tests/xwii_stubs.c \
	tests/wiilandd-smoke.sh \
	tests/wiilandd-install-smoke.sh \
	tests/cli-smoke.sh \
	tests/wiiland-config-smoke.sh \
	tools/wiilandd-hardware-report.sh \
	tools/wiiland-config.cpp \
	.github/workflows/ci.yml \
	libxwiimote.sym



#
# Build targets
#

lib_LTLIBRARIES = libxwiimote.la
bin_PROGRAMS = wiilandd
if HAVE_XWIISHOW
bin_PROGRAMS += xwiishow
endif
if HAVE_QT_UI
bin_PROGRAMS += wiiland-config
endif
bin_SCRIPTS = wiilandd-hardware-report
noinst_PROGRAMS = xwiidump
include_HEADERS = lib/xwiimote.h
man_MANS = \
	doc/wiiland.7 \
	doc/libxwiimote.7 \
	doc/wiilandd.1
if HAVE_XWIISHOW
man_MANS += doc/xwiishow.1
endif
if HAVE_QT_UI
man_MANS += doc/wiiland-config.1
endif
dist_doc_DATA = \
	doc/WIILAND


pkgconfigdir = $(libdir)/pkgconfig
pkgconfig_DATA = libxwiimote.pc
EXTRA_DIST += \
	$(pkgconfig_DATA) \
	libxwiimote.pc.in

if INSTALL_UDEV_RULES
udevrulesdir = @UDEV_RULES_DIR@
dist_udevrules_DATA = \
	res/70-udev-wiiland.rules \
	res/70-wiiland-uinput.rules
endif

if INSTALL_XORG_CONF
xorgconfdir = @XORG_CONF_DIR@
dist_xorgconf_DATA = \
	res/50-xorg-fix-wiiland.conf
endif

if INSTALL_SYSTEMD_USER_UNIT
systemduserunitdir = @SYSTEMD_USER_UNIT_DIR@
nodist_systemduserunit_DATA = \
	res/wiilandd.service
endif

wiilandconfigdir = $(sysconfdir)/wiiland
dist_wiilandconfig_DATA = \
	res/wiilandd.conf

wiilandexamplesdir = $(docdir)/examples
dist_wiilandexamples_DATA = \
	res/wiilandd.conf

if HAVE_QT_UI
applicationsdir = $(datadir)/applications
dist_applications_DATA = \
	res/io.github.philosophimoonbeam.wiiland-config.desktop
iconscalableappsdir = $(datadir)/icons/hicolor/scalable/apps
dist_iconscalableapps_DATA = \
	res/io.github.philosophimoonbeam.wiiland.svg
endif

wiilandd-hardware-report: $(srcdir)/tools/wiilandd-hardware-report.sh
	$(AM_V_GEN)$(INSTALL_SCRIPT) $(srcdir)/tools/wiilandd-hardware-report.sh $@

res/wiilandd.service: $(srcdir)/res/wiilandd.service.in Makefile
	$(AM_V_at)$(MKDIR_P) res
	$(AM_V_GEN)sed -e 's|@bindir[@]|$(bindir)|g' \
		$(srcdir)/res/wiilandd.service.in >$@

CLEANFILES = wiilandd-hardware-report
distclean-local:
	$(AM_V_at)if test "$(abs_builddir)" != "$(abs_srcdir)"; then \
		rm -f res/wiilandd.service; \
	fi


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
AM_CXXFLAGS = \
	-Wall \
	-Wextra \
	-pipe \
	-fno-common \
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
AM_CXXFLAGS += -O0 -g
else
AM_CFLAGS += -O2
AM_CXXFLAGS += -O2
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

if HAVE_QT_UI
wiiland_config_SOURCES = \
	tools/wiiland-config.cpp
wiiland_config_CPPFLAGS = \
	$(AM_CPPFLAGS) \
	$(QT6_WIDGETS_CFLAGS) \
	-DQT_NO_KEYWORDS
wiiland_config_CXXFLAGS = \
	$(AM_CXXFLAGS) \
	-std=c++17
wiiland_config_LDADD = \
	$(QT6_WIDGETS_LIBS)
wiiland_config_LDFLAGS = \
	$(AM_LDFLAGS)
endif

check_PROGRAMS = \
	libxwiimote-decode-test \
	wiilandd-uinput-test
TESTS = $(check_PROGRAMS)
libxwiimote_decode_test_SOURCES = \
	tests/libxwiimote-decode.c
libxwiimote_decode_test_CPPFLAGS = \
	$(AM_CPPFLAGS) \
	$(UDEV_CFLAGS)
libxwiimote_decode_test_LDADD = \
	$(UDEV_LIBS)
libxwiimote_decode_test_LDFLAGS = \
	$(AM_LDFLAGS)
wiilandd_uinput_test_SOURCES = \
	tests/wiilandd-uinput.c
wiilandd_uinput_test_CPPFLAGS = \
	$(AM_CPPFLAGS)
wiilandd_uinput_test_LDADD = \
	libxwiimote.la
wiilandd_uinput_test_LDFLAGS = \
	$(AM_LDFLAGS)

check_local_programs = wiilandd
if HAVE_XWIISHOW
check_local_programs += xwiishow xwiidump
endif
if HAVE_QT_UI
check_local_programs += wiiland-config
endif
check-local: $(check_local_programs)
	$(AM_V_at)./wiilandd --self-test
	$(AM_V_at)$(SHELL) $(top_srcdir)/tests/wiilandd-smoke.sh
if HAVE_XWIISHOW
	$(AM_V_at)XWIISHOW=$(abs_builddir)/xwiishow \
		XWIIDUMP=$(abs_builddir)/xwiidump \
		$(SHELL) $(top_srcdir)/tests/cli-smoke.sh
endif
if HAVE_QT_UI
	$(AM_V_at)QT_QPA_PLATFORM=offscreen WIILAND_CONFIG_SMOKE_TEST=1 \
		$(SHELL) $(top_srcdir)/tests/wiiland-config-smoke.sh offscreen \
		$(abs_builddir)/wiiland-config
endif

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
