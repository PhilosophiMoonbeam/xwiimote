#
# WiiLand - configure
# Written by David Herrmann, 2011
# Dedicated to the Public Domain
#

AC_PREREQ(2.68)

AC_INIT([wiiland],
        [2],
        [https://github.com/PhilosophiMoonbeam/wiiland/issues],
        [wiiland],
        [https://github.com/PhilosophiMoonbeam/wiiland])
AC_CONFIG_SRCDIR([lib/xwiimote.h])
AC_CONFIG_AUX_DIR([build-aux])
AC_CONFIG_MACRO_DIR([m4])
AC_CONFIG_HEADERS([config.h])
AC_USE_SYSTEM_EXTENSIONS
AC_SYS_LARGEFILE
AC_CANONICAL_HOST

AM_INIT_AUTOMAKE([foreign 1.11 subdir-objects dist-xz no-dist-gzip tar-pax -Wall -Werror -Wno-portability])
AM_SILENT_RULES([yes])

AC_ARG_ENABLE([qt-ui],
              [AS_HELP_STRING([--enable-qt-ui=@<:@auto/yes/no@:>@],
                              [build optional Qt 6 configuration UI])],
              [enable_qt_ui="$enableval"],
              [enable_qt_ui=auto])
AS_CASE([$enable_qt_ui],
        [auto|yes|no], [],
        [AC_MSG_ERROR([invalid value '$enable_qt_ui' for --enable-qt-ui; expected auto, yes, or no])])

# Don't add default compiler flags if they weren't specified
: ${CFLAGS=""}
: ${CXXFLAGS=""}

AC_PROG_CC
AM_PROG_CC_C_O
AC_PROG_CXX

AC_PROG_LN_S
m4_ifdef([AM_PROG_AR], [AM_PROG_AR])

LT_PREREQ(2.2)
LT_INIT

PKG_CHECK_MODULES([UDEV], [libudev])
AC_SUBST([UDEV_CFLAGS])
AC_SUBST([UDEV_LIBS])
AC_CHECK_HEADERS([linux/uinput.h], [],
                 [AC_MSG_ERROR([linux/uinput.h is required])])

AC_ARG_ENABLE([xwiishow],
              [AS_HELP_STRING([--enable-xwiishow=@<:@auto/yes/no@:>@],
                              [build optional ncurses xwiishow frontend])],
              [enable_xwiishow="$enableval"],
              [enable_xwiishow=auto])
AS_CASE([$enable_xwiishow],
        [auto|yes|no], [],
        [AC_MSG_ERROR([invalid value '$enable_xwiishow' for --enable-xwiishow; expected auto, yes, or no])])
have_xwiishow=no
if test "x$enable_xwiishow" != "xno"; then
        PKG_CHECK_MODULES([NCURSES], [ncurses],
                          [have_xwiishow=yes],
                          [if test "x$enable_xwiishow" = "xyes"; then
                                   AC_MSG_ERROR([ncurses is required for --enable-xwiishow=yes])
                           fi])
fi
AM_CONDITIONAL([HAVE_XWIISHOW], [test "$have_xwiishow" = yes])

have_qt_ui=no
if test "x$enable_qt_ui" != "xno"; then
        PKG_CHECK_MODULES([QT6_WIDGETS], [Qt6Widgets],
                          [save_CPPFLAGS=$CPPFLAGS
                           save_CXXFLAGS=$CXXFLAGS
                           save_LIBS=$LIBS
                           CPPFLAGS="$CPPFLAGS $QT6_WIDGETS_CFLAGS"
                           CXXFLAGS="$CXXFLAGS $QT6_WIDGETS_CFLAGS"
                           LIBS="$QT6_WIDGETS_LIBS $LIBS"
                           AC_LANG_PUSH([C++])
                           AC_LINK_IFELSE(
                                   [AC_LANG_PROGRAM(
                                           [[#include <QApplication>
#include <QWidget>]],
                                           [[QWidget widget;
widget.setWindowTitle(QStringLiteral("WiiLand"));]])],
                                   [have_qt_ui=yes],
                                   [have_qt_ui=no])
                           AC_LANG_POP([C++])
                           CPPFLAGS=$save_CPPFLAGS
                           CXXFLAGS=$save_CXXFLAGS
                           LIBS=$save_LIBS],
                          [have_qt_ui=no])
        if test "x$enable_qt_ui" = "xyes" && test "$have_qt_ui" != yes; then
                AC_MSG_ERROR([Qt6Widgets headers and a working C++ compiler are required for --enable-qt-ui=yes])
        fi
fi
AM_CONDITIONAL([HAVE_QT_UI], [test "$have_qt_ui" = yes])
if test "$have_qt_ui" = yes; then
        qt_qpa_runtime="not checked (Wayland and xcb plugins are loaded at runtime)"
        AC_MSG_NOTICE([Qt QPA runtime availability is not checked; install the appropriate Wayland or xcb platform plugin])
else
        qt_qpa_runtime="not applicable (Qt UI disabled)"
        AC_MSG_NOTICE([Qt QPA runtime availability is not applicable because the Qt UI is disabled])
fi

AC_ARG_WITH([udev-rules-dir],
            [AS_HELP_STRING([--with-udev-rules-dir=@<:@auto/no/PATH@:>@],
                            [install udev rules in PATH])],
            [with_udev_rules_dir="$withval"],
            [with_udev_rules_dir=auto])
AS_CASE([$with_udev_rules_dir],
        [auto], [UDEV_RULES_DIR=`$PKG_CONFIG --variable=udevdir udev 2>/dev/null`
                 if test -n "$UDEV_RULES_DIR"; then
                         UDEV_RULES_DIR="$UDEV_RULES_DIR/rules.d"
                 else
                         UDEV_RULES_DIR='${prefix}/lib/udev/rules.d'
                 fi],
        [no], [UDEV_RULES_DIR=],
        [yes|''], [AC_MSG_ERROR([--with-udev-rules-dir requires auto, no, or an absolute PATH])],
        [/*], [UDEV_RULES_DIR="$with_udev_rules_dir"],
        [AC_MSG_ERROR([--with-udev-rules-dir requires auto, no, or an absolute PATH])])
AC_SUBST([UDEV_RULES_DIR])
AM_CONDITIONAL([INSTALL_UDEV_RULES], [test "x$with_udev_rules_dir" != "xno"])

AC_ARG_WITH([systemd-user-unit-dir],
            [AS_HELP_STRING([--with-systemd-user-unit-dir=@<:@auto/no/PATH@:>@],
                            [install the systemd user unit in PATH])],
            [with_systemd_user_unit_dir="$withval"],
            [with_systemd_user_unit_dir=auto])
AS_CASE([$with_systemd_user_unit_dir],
        [auto], [SYSTEMD_USER_UNIT_DIR=`$PKG_CONFIG --variable=systemduserunitdir systemd 2>/dev/null`
                 if test -z "$SYSTEMD_USER_UNIT_DIR"; then
                         SYSTEMD_USER_UNIT_DIR='${prefix}/lib/systemd/user'
                 fi],
        [no], [SYSTEMD_USER_UNIT_DIR=],
        [yes|''], [AC_MSG_ERROR([--with-systemd-user-unit-dir requires auto, no, or an absolute PATH])],
        [/*], [SYSTEMD_USER_UNIT_DIR="$with_systemd_user_unit_dir"],
        [AC_MSG_ERROR([--with-systemd-user-unit-dir requires auto, no, or an absolute PATH])])
AC_SUBST([SYSTEMD_USER_UNIT_DIR])
AM_CONDITIONAL([INSTALL_SYSTEMD_USER_UNIT], [test "x$with_systemd_user_unit_dir" != "xno"])

AC_ARG_WITH([xorg-conf-dir],
            [AS_HELP_STRING([--with-xorg-conf-dir=@<:@auto/no/PATH@:>@],
                            [install the X.org policy in PATH])],
            [with_xorg_conf_dir="$withval"],
            [with_xorg_conf_dir=auto])
AS_CASE([$with_xorg_conf_dir],
        [auto], [XORG_CONF_DIR=`$PKG_CONFIG --variable=sysconfigdir xorg-server 2>/dev/null`
                 if test -z "$XORG_CONF_DIR"; then
                         XORG_CONF_DIR='${datadir}/X11/xorg.conf.d'
                 fi],
        [no], [XORG_CONF_DIR=],
        [yes|''], [AC_MSG_ERROR([--with-xorg-conf-dir requires auto, no, or an absolute PATH])],
        [/*], [XORG_CONF_DIR="$with_xorg_conf_dir"],
        [AC_MSG_ERROR([--with-xorg-conf-dir requires auto, no, or an absolute PATH])])
AC_SUBST([XORG_CONF_DIR])
AM_CONDITIONAL([INSTALL_XORG_CONF], [test "x$with_xorg_conf_dir" != "xno"])

AC_MSG_CHECKING([whether to build with debugging on])
AC_ARG_ENABLE([debug],
              [AS_HELP_STRING([--enable-debug], [whether to build with debugging on)])],
              [debug="$enableval"],
              [debug=no; AC_DEFINE([NDEBUG], [1], [No Debug])])
AM_CONDITIONAL([DEBUG], [test x$debug = xyes])
AC_MSG_RESULT([$debug])

#
# Check for doxygen
#

AC_ARG_VAR([DOXYGEN], [Path to doxygen command])
AC_ARG_WITH(doxygen,
            AS_HELP_STRING([--with-doxygen],
                           [Use doxygen to generate documentation]),
            [use_doxygen=$withval],
            [use_doxygen=auto])
have_doxygen=no
if test ! "x$use_doxygen" = "xno" ; then
        AC_PATH_PROG([DOXYGEN], [doxygen])
        if test "x$DOXYGEN" = "x" ; then
                if test "x$use_doxygen" = "xyes" ; then
                        AC_MSG_ERROR([--with-doxygen=yes specified but doxygen not found in PATH])
                fi
        else
                have_doxygen=yes
        fi
fi
AM_CONDITIONAL([HAVE_DOXYGEN], [test "$have_doxygen" = yes])

#
# Write configuration
#

AC_CONFIG_FILES([Makefile libxwiimote.pc doc/Doxyfile])
AC_OUTPUT

AC_MSG_NOTICE([Build configuration:

               prefix: $prefix
          exec-prefix: $exec_prefix
               libdir: $libdir
           includedir: $includedir

                debug: $debug
              doxygen: $have_doxygen
            xwiishow: $have_xwiishow
               qt-ui: $have_qt_ui
       Qt QPA runtime: $qt_qpa_runtime

           udev rules: ${UDEV_RULES_DIR:-no}
    systemd user unit: ${SYSTEMD_USER_UNIT_DIR:-no}
       X.org conf dir: ${XORG_CONF_DIR:-no}

        Run "${MAKE-make}" to start compilation process])
