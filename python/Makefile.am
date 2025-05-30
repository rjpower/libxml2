# Makefile for libxml2 python library

# We use a rule with multiple output files which creates problems with
# parallel builds.
.NOTPARALLEL:

SUBDIRS = . tests

EXTRA_DIST = 			\
	generator.py		\
	libxml.py		\
	pyproject.toml		\
	meson.build

if WITH_PYTHON
AM_CPPFLAGS = \
	-I$(top_builddir)/include \
	-I$(top_srcdir)/include	\
	$(PYTHON_CFLAGS)

pyexec_LTLIBRARIES = libxml2mod.la

libxml2mod_la_SOURCES = libxml.c libxml_wrap.h types.c
nodist_libxml2mod_la_SOURCES = libxml2-py.h libxml2-py.c
libxml2mod_la_LDFLAGS = $(AM_LDFLAGS) $(PYTHON_LDFLAGS) -module -avoid-version
libxml2mod_la_LIBADD = $(top_builddir)/libxml2.la $(PYTHON_LIBS)

BUILT_SOURCES = libxml2-export.c libxml2-py.h libxml2-py.c

python_PYTHON = drv_libxml2.py
nodist_python_PYTHON = libxml2.py

API_DESC = ../doc/html.stamp
GENERATED = libxml2.py $(BUILT_SOURCES)
CLEANFILES = $(GENERATED)

all-local: libxml2.py

$(GENERATED): $(srcdir)/generator.py $(API_DESC)
	$(PYTHON) $(srcdir)/generator.py $(builddir)

# libxml.c #includes libxml2-export.c
libxml.$(OBJEXT): libxml2-export.c

clean-local:
	rm -rf __pycache__ *.pyc

endif
