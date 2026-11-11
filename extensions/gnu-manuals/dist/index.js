// dev.raygun.gnu-manuals 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };

  // src/manuals.ts
  var MANUALS = [
    {
      id: "bash",
      title: "Bash Reference Manual",
      name: "Bash",
      description: "The GNU Bourne-Again SHell.",
      category: "Software development",
      path: "/software/bash/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "coreutils",
      title: "GNU Coreutils",
      name: "Coreutils",
      description: "Core GNU utilities (file, text, shell).",
      category: "Software development",
      path: "/software/coreutils/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-emacs",
      title: "GNU Emacs Manual",
      name: "Emacs (emacs)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/emacs/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "gawk",
      title: "The GNU Awk User’s Guide",
      name: "Gawk",
      description: "Text scanning and processing language.",
      category: "Text creation and manipulation",
      path: "/software/gawk/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "sed",
      title: "sed, a stream editor",
      name: "Sed",
      description: "Stream editor.",
      category: "Text creation and manipulation",
      path: "/software/sed/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "grep",
      title: "GNU Grep",
      name: "Grep",
      description: "Print lines matching a pattern.",
      category: "Text creation and manipulation",
      path: "/software/grep/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "make",
      title: "GNU make",
      name: "Make",
      description: "Remake files automatically.",
      category: "Software development",
      path: "/software/make/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "tar",
      title: "GNU tar",
      name: "Tar",
      description: "Managing tar archives.",
      category: "Archiving",
      path: "/software/tar/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "gzip",
      title: "GNU Gzip",
      name: "Gzip",
      description: "General file (de)compression (using lzw).",
      category: "Archiving",
      path: "/software/gzip/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "wget",
      title: "GNU Wget Manual",
      name: "Wget",
      description: "Non-interactive command-line utility for downloading files.",
      category: "Internet applications",
      path: "/software/wget/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "diffutils",
      title: "Comparing and Merging Files",
      name: "Diffutils",
      description: "Comparing and merging files.",
      category: "Text creation and manipulation",
      path: "/software/diffutils/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "findutils",
      title: "GNU Findutils",
      name: "Findutils",
      description: "Operating on files matching given criteria.",
      category: "Software development",
      path: "/software/findutils/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-elisp",
      title: "GNU Emacs Lisp Reference Manual",
      name: "Emacs (elisp)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/elisp/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-eintr",
      title: "Programming in Emacs Lisp",
      name: "Emacs (eintr)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/eintr/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "c-intro-and-ref",
      title: "GNU C Language Manual",
      name: "GNU C Introduction and Reference",
      description: "Introduction for learning the GNU C language, and reference for it.",
      category: "Software development",
      path: "/software/c-intro-and-ref/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-org",
      title: "The Org Manual",
      name: "Emacs (org)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/org/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "m4",
      title: "GNU M4 macro processor",
      name: "M4",
      description: "Macro processor.",
      category: "Text creation and manipulation",
      path: "/software/m4/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "autoconf",
      title: "Autoconf",
      name: "Autoconf",
      description: "Create source code configuration scripts.",
      category: "Software development",
      path: "/software/autoconf/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "automake",
      title: "automake",
      name: "Automake",
      description: "Making GNU standards-compliant Makefiles.",
      category: "Software development",
      path: "/software/automake/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "libtool",
      title: "Libtool",
      name: "Libtool",
      description: "Generic shared library support tools.",
      category: "Software libraries",
      path: "/software/libtool/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "bison",
      title: "Bison",
      name: "Bison",
      description: "Parser generator.",
      category: "Software development",
      path: "/software/bison/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "texinfo-texinfo",
      title: "GNU Texinfo",
      name: "Texinfo (texinfo)",
      description: "Part of the Texinfo documentation. The GNU documentation format.",
      category: "Text creation and manipulation",
      path: "/software/texinfo/manual/texinfo/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "groff",
      title: "The GNU troff Manual",
      name: "Groff",
      description: "Typesetting from plain text mixed with formatting commands.",
      category: "Text creation and manipulation",
      path: "/software/groff/manual/groff.html.node/",
      licence: "GNU Free Documentation License 1.3 or later, or the GNU General Public License"
    },
    {
      id: "units",
      title: "Units: A Unit Conversion Program and Scientific Calculator",
      name: "Units",
      description: "Conversion between thousands of scales.",
      category: "Science",
      path: "/software/units/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "inetutils",
      title: "GNU Inetutils",
      name: "Inetutils",
      description: "Basic networking utilities.",
      category: "Internet applications",
      path: "/software/inetutils/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "gnulib",
      title: "GNU Gnulib",
      name: "GNUlib",
      description: "Source files to share among distributions.",
      category: "Software development",
      path: "/software/gnulib/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-calc",
      title: "GNU Emacs Calc Manual",
      name: "Emacs (calc)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/calc/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "recutils",
      title: "GNU Recutils",
      name: "Recutils",
      description: "Manipulate plain text files as databases.",
      category: "Database",
      path: "/software/recutils/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "pspp",
      title: "PSPP",
      name: "PSPP",
      description: "Statistical analysis.",
      category: "Mathematics",
      path: "/software/pspp/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "chess",
      title: "GNU Chess",
      name: "Chess",
      description: "Full chess implementation.",
      category: "Games",
      path: "/software/chess/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "pkg_8sync",
      title: "8sync",
      name: "8sync",
      description: "An asynchronous programming library for Guile.",
      category: "Software libraries",
      path: "/software/8sync/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-ebrowse",
      title: "A Class Browser for C++",
      name: "Emacs (ebrowse)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/ebrowse/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "wdiff",
      title: "A word difference finder",
      name: "Wdiff",
      description: "Word difference finder.",
      category: "Text creation and manipulation",
      path: "/software/wdiff/manual/html_node/",
      licence: "GNU permissive documentation licence"
    },
    {
      id: "emacs-bovine",
      title: "Bovine parser development",
      name: "Emacs (bovine)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/bovine/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "bpel2owfn",
      title: "BPEL2oWFN",
      name: "BPEL2oWFN",
      description: "Translating web service in BPEL to Open Workflow Nets.",
      category: "Business and productivity",
      path: "/software/bpel2owfn/manual/2.0.x/bpel2owfn.html",
      licence: "GNU Free Documentation License 1.2 or later"
    },
    {
      id: "emacs-ccmode",
      title: "CC Mode Manual",
      name: "Emacs (ccmode)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/ccmode/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "ccd2cue",
      title: "Ccd2cue",
      name: "Ccd2cue",
      description: "CCD to CUE sheet conversion.",
      category: "Audio",
      path: "/software/ccd2cue/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-cl",
      title: "Common Lisp Extensions",
      name: "Emacs (cl)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/cl/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "dejagnu",
      title: "DejaGnu",
      name: "DejaGnu",
      description: "GNU software testing framework.",
      category: "Software development",
      path: "/software/dejagnu/manual/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-dired-x",
      title: "Dired Extra User’s Manual",
      name: "Emacs (dired-x)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/dired-x/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-epa",
      title: "EasyPG Assistant User’s Manual",
      name: "Emacs (epa)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/epa/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-ediff",
      title: "Ediff User’s Manual",
      name: "Emacs (ediff)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/ediff/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-edt",
      title: "EDT Emulation for Emacs",
      name: "Emacs (edt)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/edt/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-eglot",
      title: "Eglot: The Emacs Client for the Language Server Protocol",
      name: "Emacs (eglot)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/eglot/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-auth",
      title: "Emacs auth-source Library",
      name: "Emacs (auth)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/auth/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-ede",
      title: "Emacs Development Environment",
      name: "Emacs (ede)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/ede/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-emacs-gnutls",
      title: "Emacs GnuTLS Integration",
      name: "Emacs (emacs-gnutls)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/emacs-gnutls/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-mairix-el",
      title: "Emacs Interface for Mairix",
      name: "Emacs (mairix-el)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/mairix-el/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-ert",
      title: "Emacs Lisp Regression Testing",
      name: "Emacs (ert)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/ert/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-emacs-mime",
      title: "Emacs MIME Manual",
      name: "Emacs (emacs-mime)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/emacs-mime/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-sasl",
      title: "Emacs SASL Library",
      name: "Emacs (sasl)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/sasl/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-sieve",
      title: "Emacs Sieve Manual",
      name: "Emacs (sieve)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/sieve/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-smtpmail",
      title: "Emacs SMTP Library",
      name: "Emacs (smtpmail)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/smtpmail/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-eudc",
      title: "Emacs Unified Directory Client (EUDC) Manual",
      name: "Emacs (eudc)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/eudc/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-eww",
      title: "Emacs Web Wowser",
      name: "Emacs (eww)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/eww/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-muse",
      title: "Emacs-muse",
      name: "Emacs-muse",
      description: "Authoring and publishing environment for Emacs.",
      category: "Web authoring",
      path: "/software/emacs-muse/manual/html_node/",
      licence: "GNU Free Documentation License 1.2 or later"
    },
    {
      id: "emms",
      title: "EMMS",
      name: "EMMS",
      description: "Emacs Multimedia System.",
      category: "Audio",
      path: "/software/emms/manual/",
      licence: "GNU Free Documentation License 1.1 or later"
    },
    {
      id: "emacs-eieio",
      title: "Enhanced Implementation of Emacs Interpreted Objects",
      name: "Emacs (eieio)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/eieio/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-erc",
      title: "ERC Manual",
      name: "Emacs (erc)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/erc/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-eshell",
      title: "Eshell: The Emacs Shell",
      name: "Emacs (eshell)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/eshell/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-autotype",
      title: "Features for Automatic Typing",
      name: "Emacs (autotype)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/autotype/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-forms",
      title: "Forms Mode User’s Manual",
      name: "Emacs (forms)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/forms/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "gmediaserver",
      title: "Gmediaserver",
      name: "Gmediaserver",
      description: "UPnP compatible media server.",
      category: "Audio",
      path: "/software/gmediaserver/manual/",
      licence: "GNU Free Documentation License 1.2 or later, or the GNU General Public License"
    },
    {
      id: "alive",
      title: "GNU Alive",
      name: "Alive",
      description: "Autologin and keep-alive daemon.",
      category: "Internet applications",
      path: "/software/alive/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "cflow",
      title: "GNU cflow",
      name: "Cflow",
      description: "Create a graph of control flow within a program.",
      category: "Software development",
      path: "/software/cflow/manual/html_node/",
      licence: "GNU Free Documentation License 1.2 or later"
    },
    {
      id: "emacs-flymake",
      title: "GNU Flymake",
      name: "Emacs (flymake)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/flymake/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "gss",
      title: "GNU Generic Security Service Library",
      name: "Generic Security Service",
      description: "Generic Security Service library.",
      category: "Software libraries",
      path: "/software/gss/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "gv",
      title: "GNU gv Manual",
      name: "Gv",
      description: "PostScript and PDF viewer using Ghostscript as a back-end.",
      category: "Printing",
      path: "/software/gv/manual/html_node/",
      licence: "GNU General Public License 3 or later"
    },
    {
      id: "hello",
      title: "GNU Hello",
      name: "Hello",
      description: "Hello, GNU world: An example GNU package.",
      category: "Software development",
      path: "/software/hello/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "libidn",
      title: "GNU Libidn",
      name: "Libidn",
      description: "Internationalized string processing library.",
      category: "Software libraries",
      path: "/software/libidn/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "librejs",
      title: "GNU LibreJS",
      name: "Librejs",
      description: "Browser add-on that blocks nonfree nontrivial JavaScript.",
      category: "Internet applications",
      path: "/software/librejs/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "libtasn1",
      title: "GNU Libtasn1",
      name: "Libtasn1",
      description: "ASN.1 library.",
      category: "Security",
      path: "/software/libtasn1/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "mes",
      title: "GNU Mes Reference Manual",
      name: "Mes",
      description: "Full source bootstrapping for GNU systems.",
      category: "System administration",
      path: "/software/mes/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "mdk",
      title: "GNU MIX Development Kit (mdk)",
      name: "MIX Development Kit",
      description: "Virtual development environment for Knuth's MIX.",
      category: "Education",
      path: "/software/mdk/manual/html_node/",
      licence: "GNU Free Documentation License 1.2 or later"
    },
    {
      id: "orgadoc",
      title: "GNU OrgaDoc Manual",
      name: "OrgaDoc",
      description: "Eiffel program to maintain and query documents across machines.",
      category: "Text creation and manipulation",
      path: "/software/orgadoc/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "radius",
      title: "GNU Radius Reference Manual",
      name: "Radius",
      description: "Authentication and accounting services and daemon.",
      category: "Security",
      path: "/software/radius/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "rcs",
      title: "GNU RCS",
      name: "RCS",
      description: "Per-file local revision control system.",
      category: "Version control",
      path: "/software/rcs/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "serveez",
      title: "GNU Serveez",
      name: "Serveez",
      description: "Framework for implementing IP-based servers.",
      category: "System administration",
      path: "/software/serveez/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "gsasl",
      title: "GNU Simple Authentication and Security Layer",
      name: "Gsasl",
      description: "Simple Authentication and Security Layer library.",
      category: "Software libraries",
      path: "/software/gsasl/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "gsrc",
      title: "GNU Source Release Collection",
      name: "GNU Source Release Collection",
      description: "Install latest official GNU releases from original source.",
      category: "System administration",
      path: "/software/gsrc/manual/html_node/",
      licence: "GNU Free Documentation License 1.2 or later"
    },
    {
      id: "texinfo-texi2any-api",
      title: "GNU Texinfo texi2any Output Customization",
      name: "Texinfo (texi2any_api)",
      description: "Part of the Texinfo documentation. The GNU documentation format.",
      category: "Text creation and manipulation",
      path: "/software/texinfo/manual/texi2any_api/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "gnun-web-trans",
      title: "GNU Web Translators Manual",
      name: "GNUnited Nations (web-trans)",
      description: "Part of the GNUnited Nations documentation. Build system for www.gnu.org translations.",
      category: "Documentation translation",
      path: "/software/trans-coord/manual/web-trans/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "gnun-gnun",
      title: "GNUnited Nations",
      name: "GNUnited Nations (gnun)",
      description: "Part of the GNUnited Nations documentation. Build system for www.gnu.org translations.",
      category: "Documentation translation",
      path: "/software/trans-coord/manual/gnun/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "guile-ncurses",
      title: "Guile-ncurses",
      name: "Guile-ncurses",
      description: "Guile bindings to ncurses.",
      category: "Interface",
      path: "/software/guile-ncurses/manual/html_node/",
      licence: "MIT-style licence"
    },
    {
      id: "guile-rpc",
      title: "Guile-rpc",
      name: "Guile-rpc",
      description: "Pure Scheme implementation for the ONC RPC standard.",
      category: "Software development",
      path: "/software/guile-rpc/manual/html_node/",
      licence: "GNU Free Documentation License 1.1 or later"
    },
    {
      id: "guile-sdl",
      title: "Guile-SDL",
      name: "Guile-SDL",
      description: "Guile interface for SDL (Simple DirectMedia Layer).",
      category: "Audio",
      path: "/software/guile-sdl/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-htmlfontify",
      title: "Htmlfontify User Manual",
      name: "Emacs (htmlfontify)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/htmlfontify/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-idlwave",
      title: "IDLWAVE User Manual",
      name: "Emacs (idlwave)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/idlwave/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-info",
      title: "Info",
      name: "Emacs (info)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/info/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-ido",
      title: "Interactive Do",
      name: "Emacs (ido)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/ido/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "jtw",
      title: "Java Training Wheels",
      name: "Java Training Wheels",
      description: "A simplified Java environment for learning programming.",
      category: "Education",
      path: "/software/jtw/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "libextractor",
      title: "Libextractor",
      name: "Libextractor",
      description: "Library to extract meta-data from media files.",
      category: "Audio",
      path: "/software/libextractor/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "liquidwar6",
      title: "Liquid War 6",
      name: "Liquid War 6",
      description: "Multiplayer wargame where your army is a blob of liquid.",
      category: "Games",
      path: "/software/liquidwar6/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-message",
      title: "Message Manual",
      name: "Emacs (message)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/message/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "mtools",
      title: "Mtools",
      name: "Mtools",
      description: "Access MS-DOS disks without mounting.",
      category: "System administration",
      path: "/software/mtools/manual/mtools.html",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-newsticker",
      title: "Newsticker",
      name: "Emacs (newsticker)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/newsticker/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-nxml-mode",
      title: "nXML Mode",
      name: "Emacs (nxml-mode)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/nxml-mode/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-octave-mode",
      title: "Octave Mode",
      name: "Emacs (octave-mode)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/octave-mode/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-pcl-cvs",
      title: "PCL-CVS—Emacs Front-End to CVS",
      name: "Emacs (pcl-cvs)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/pcl-cvs/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-pgg",
      title: "PGG",
      name: "Emacs (pgg)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/pgg/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "proxyknife",
      title: "Proxyknife",
      name: "Proxyknife",
      description: "Customizable multithread proxy hunter.",
      category: "Internet applications",
      path: "/software/proxyknife/manual/html_node/",
      licence: "GNU Free Documentation License 1.2 or later"
    },
    {
      id: "emacs-rcirc",
      title: "rcirc Manual",
      name: "Emacs (rcirc)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/rcirc/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-reftex",
      title: "RefTeX User Manual",
      name: "Emacs (reftex)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/reftex/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-remember",
      title: "Remember Manual",
      name: "Emacs (remember)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/remember/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "remotecontrol",
      title: "Remotecontrol",
      name: "Remotecontrol",
      description: "Management of IP-enabled thermostats and other HVAC devices.",
      category: "Interface",
      path: "/software/remotecontrol/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-semantic",
      title: "Semantic Manual",
      name: "Emacs (semantic)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/semantic/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-ses",
      title: "SES: Simple Emacs Spreadsheet",
      name: "Emacs (ses)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/ses/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "sharutils",
      title: "Sharutils",
      name: "Sharutils",
      description: "Archives in shell scripts, uuencode/uudecode.",
      category: "Archiving",
      path: "/software/sharutils/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-speedbar",
      title: "Speedbar: File/Tag summarizing utility",
      name: "Emacs (speedbar)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/speedbar/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "texinfo-info-stnd",
      title: "Stand-alone GNU Info",
      name: "Texinfo (info-stnd)",
      description: "Part of the Texinfo documentation. The GNU documentation format.",
      category: "Text creation and manipulation",
      path: "/software/texinfo/manual/info-stnd/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "stow",
      title: "Stow",
      name: "Stow",
      description: "Managing installed software packages.",
      category: "System administration",
      path: "/software/stow/manual/html_node/",
      licence: "GNU permissive documentation licence"
    },
    {
      id: "emacs-sc",
      title: "Supercite User’s Manual",
      name: "Emacs (sc)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/sc/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "gawk-gawkinet",
      title: "TCP/IP Internetworking With gawk",
      name: "Gawk (gawkinet)",
      description: "Part of the Gawk documentation. Text scanning and processing language.",
      category: "Text creation and manipulation",
      path: "/software/gawk/manual/gawkinet/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "teseq",
      title: "Teseq",
      name: "Teseq",
      description: "Analyze files with terminal control sequences, for debugging.",
      category: "Software development",
      path: "/software/teseq/manual/html_node/",
      licence: "GNU Free Documentation License 1.2 or later"
    },
    {
      id: "emacs-widget",
      title: "The Emacs Widget Library",
      name: "Emacs (widget)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/widget/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "libmicrohttpd",
      title: "The GNU libmicrohttpd Reference Manual",
      name: "Libmicrohttpd",
      description: "C library implementing an HTTP 1.1 server.",
      category: "Software libraries",
      path: "/software/libmicrohttpd/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-mh-e",
      title: "The MH-E Manual",
      name: "Emacs (mh-e)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/mh-e/",
      licence: "GNU Free Documentation License 1.3 or later, or the GNU General Public License"
    },
    {
      id: "emacs-todo-mode",
      title: "Todo Mode User Manual",
      name: "Emacs (todo-mode)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/todo-mode/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-tramp",
      title: "TRAMP User Manual",
      name: "Emacs (tramp)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/tramp/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-url",
      title: "URL Programmer’s Manual",
      name: "Emacs (url)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/url/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-use-package",
      title: "use-package User Manual",
      name: "Emacs (use-package)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/use-package/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-dbus",
      title: "Using of D-Bus",
      name: "Emacs (dbus)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/dbus/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "vc-dwim",
      title: "vc-dwim",
      name: "Vc-dwim",
      description: "Version-control-agnostic ChangeLog diff and commit tool.",
      category: "Version control",
      path: "/software/vc-dwim/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-vhdl-mode",
      title: "VHDL Mode, an Emacs mode for editing VHDL code",
      name: "Emacs (vhdl-mode)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/vhdl-mode/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-vip",
      title: "VIP",
      name: "Emacs (vip)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/vip/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-viper",
      title: "Viper Is a Package for Emacs Rebels",
      name: "Emacs (viper)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/viper/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "websocket4j",
      title: "Websocket4j",
      name: "Websocket4j",
      description: "WebSocket protocol implementation for Java-HTML5 communication.",
      category: "Internet applications",
      path: "/software/websocket4j/manual/html_node/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-wisent",
      title: "Wisent Parser Development",
      name: "Emacs (wisent)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/wisent/",
      licence: "GNU Free Documentation License 1.3 or later"
    },
    {
      id: "emacs-woman",
      title: "WoMan: Browse Unix Manual Pages “W.O. (without) Man”",
      name: "Emacs (woman)",
      description: "Part of the Emacs documentation. The extensible, customizable, self-documenting text editor.",
      category: "Editors",
      path: "/software/emacs/manual/html_node/woman/",
      licence: "GNU Free Documentation License 1.3 or later"
    }
  ];

  // src/index.ts
  var SITE = "https://www.gnu.org";
  var PAGE_SIZE = 30;
  var HEADERS = { "User-Agent": "Raygun/1.0 ( https://github.com/uvxszi/raygun )" };
  var PAGE = /^[\w.-]+\.html?$/;
  var ANCHOR = /^[\w.:-]*$/;
  var DETAILS_WAIT_MS = 8e3;
  var DROPPED_CLASSES = /* @__PURE__ */ new Set([
    "nav-panel",
    "header",
    "node",
    "menu",
    "mini-toc",
    "contents",
    "shortcontents",
    "region-contents",
    "copiable-link",
    "copiable-anchor"
  ]);
  var CATEGORIES = [
    { value: "all", label: "All" },
    ...[...new Set(MANUALS.map((manual) => manual.category))].sort((first, second) => first.localeCompare(second)).map((category) => ({ value: category, label: category }))
  ];
  var SORTS = [
    { value: "known", label: "Best known" },
    { value: "title", label: "Title (A–Z)" }
  ];
  var decode = (text) => text.replace(/&nbsp;|&#160;/g, " ").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/&#0?39;|&apos;/g, "'").replace(/&rsquo;/g, "’").replace(/&lsquo;/g, "‘").replace(/&ldquo;/g, "“").replace(/&rdquo;/g, "”").replace(/&ndash;/g, "–").replace(/&mdash;/g, "—").replace(/&copy;/g, "©").replace(
    /&#x([0-9a-f]+);/gi,
    (_match, code) => String.fromCodePoint(parseInt(code, 16))
  ).replace(/&#(\d+);/g, (_match, code) => String.fromCodePoint(Number(code))).replace(/&amp;/g, "&");
  var plainText = (html) => decode(html.replace(/<[^>]+>/g, " ")).replace(/\s+/g, " ").trim();
  var words = (text) => text.toLowerCase().replace(/[^\p{L}\p{N}]+/gu, " ").trim();
  var VOID_ELEMENTS = /* @__PURE__ */ new Set([
    "area",
    "base",
    "br",
    "col",
    "embed",
    "hr",
    "img",
    "input",
    "link",
    "meta",
    "source",
    "track",
    "wbr"
  ]);
  var TAG = /<(\/?)([a-zA-Z][\w:-]*)((?:[^>"']|"[^"]*"|'[^']*')*)>/g;
  var dropElements = (html, drop) => {
    let kept = "";
    let from = 0;
    let open = null;
    for (const match of html.matchAll(TAG)) {
      const [whole, closing, name = "", attributes = ""] = match;
      const tag = name.toLowerCase();
      const at = match.index ?? 0;
      const lone = VOID_ELEMENTS.has(tag) || attributes.trimEnd().endsWith("/");
      if (open) {
        if (tag === open.tag && !lone) open.depth += closing ? -1 : 1;
        if (open.depth === 0) {
          open = null;
          from = at + whole.length;
        }
        continue;
      }
      if (closing || !drop(tag, attributes)) continue;
      kept += html.slice(from, at);
      from = at + whole.length;
      if (!lone) open = { tag, depth: 1 };
    }
    return open ? kept : kept + html.slice(from);
  };
  var classes = (attributes) => /\bclass\s*=\s*(?:"([^"]*)"|'([^']*)')/i.exec(attributes)?.slice(1).join(" ").split(/\s+/).filter(Boolean) ?? [];
  var turn = Promise.resolve();
  var inTurn = (task) => {
    const next = turn.then(task);
    turn = next.catch(() => void 0);
    return next;
  };
  var fetchPage = (url) => inTurn(async () => {
    let response;
    try {
      response = await raygun.http.fetch(url, { headers: HEADERS });
    } catch {
      throw new Error("www.gnu.org didn't answer in time; try again in a minute");
    }
    if (response.status === 404)
      throw new Error("www.gnu.org no longer has this page of the manual");
    if (!response.ok)
      throw new Error(`www.gnu.org answered with an error (HTTP ${response.status})`);
    return { text: await response.text(), url: response.url };
  });
  var pages = /* @__PURE__ */ new Map();
  var cachedPage = (url) => {
    let found = pages.get(url);
    if (!found) {
      found = fetchPage(url);
      found.catch(() => pages.delete(url));
      pages.set(url, found);
      if (pages.size > 4) pages.delete(pages.keys().next().value ?? url);
    }
    return found;
  };
  var manualOf = (id) => {
    const manual = MANUALS.find((known) => known.id === id);
    if (!manual) throw new Error("Raygun lists no GNU manual by this name");
    return manual;
  };
  var placeOf = (manual) => {
    const slash = manual.path.lastIndexOf("/");
    return {
      folder: manual.path.slice(0, slash + 1),
      top: manual.path.slice(slash + 1) || "index.html"
    };
  };
  var pageUrl = (manual, file) => `${SITE}${placeOf(manual).folder}${file}`;
  var noticeOf = (html) => {
    for (const comment of html.matchAll(/<!--([\s\S]*?)-->/g)) {
      const text = comment[1] ?? "";
      if (/permission|licen[cs]e/i.test(text)) return plainText(text);
    }
    const quote = /<blockquote[^>]*>([\s\S]*?)<\/blockquote>/.exec(html)?.[1] ?? "";
    return /permission|licen[cs]e/i.test(quote) ? plainText(quote) : "";
  };
  var licenceOf = (notice) => {
    const later = /any later version/i.test(notice) ? " or later" : "";
    const gpl = /GNU General Public License/i.test(notice);
    const fdl = /GNU Free Documentation License,?\s*(?:Version\s*)?(\d\.\d)?/i.exec(notice);
    if (fdl) {
      const name = `GNU Free Documentation License${fdl[1] ? ` ${fdl[1]}` : ""}${later}`;
      return gpl && /either|dual/i.test(notice) ? `${name}, or the GNU General Public License` : name;
    }
    if (/Permission is granted to make and distribute verbatim copies/i.test(notice) && /Permission is granted to copy and distribute modified versions/i.test(notice)) {
      return "GNU permissive documentation licence";
    }
    if (gpl) {
      const version = /General Public License[^.]*?version (\d)/i.exec(notice)?.[1];
      return `GNU General Public License${version ? ` ${version}` : ""}${later}`;
    }
    const creativeCommons = /creativecommons\.org\/licenses\/([a-z-]+)\/(\d\.\d)/i.exec(notice);
    if (creativeCommons) {
      return `CC ${(creativeCommons[1] ?? "").toUpperCase()} ${creativeCommons[2] ?? ""}`;
    }
    return /Permission is hereby granted, free of charge/i.test(notice) ? "MIT-style licence" : null;
  };
  var freeLicence = (top) => {
    const licence = licenceOf(noticeOf(top.text));
    if (!licence) throw new Error("This manual's copying notice doesn't grant a free licence");
    return licence;
  };
  var introOf = (html) => {
    const body = /<body[^>]*>([\s\S]*)/i.exec(html)?.[1] ?? html;
    for (const match of body.matchAll(/<p>([\s\S]*?)<\/p>/g)) {
      const text = plainText(match[1] ?? "");
      if (text.length > 40 && !/^(?:Copyright|Permission|Next:|Up:|Previous:)/i.test(text)) {
        return text.length > 600 ? `${text.slice(0, 597)}…` : text;
      }
    }
    return "";
  };
  var elementFrom = (html, at) => {
    let depth = 0;
    let tag = "";
    for (const match of html.slice(at).matchAll(TAG)) {
      const [whole, closing, name = ""] = match;
      const current = name.toLowerCase();
      if (!tag) tag = current;
      if (current !== tag || VOID_ELEMENTS.has(current)) continue;
      depth += closing ? -1 : 1;
      if (depth === 0) return html.slice(at, at + (match.index ?? 0) + whole.length);
    }
    return html.slice(at);
  };
  var tocEntries = (html, top) => {
    const at = html.search(/<div class="contents"/);
    const contents = at >= 0 ? elementFrom(html, at) : html;
    const links = [...contents.matchAll(/<a\b([^>]*)>([\s\S]*?)<\/a>/g)].filter(
      (match) => at >= 0 ? true : /\b(?:id|name)="toc[-_]/.test(match[1] ?? "")
    );
    return links.flatMap((match) => {
      const href = /\bhref="([^"]*)"/.exec(match[1] ?? "")?.[1];
      if (href === void 0) return [];
      const [file = "", anchor = ""] = decode(href).split("#");
      const title = plainText(match[2] ?? "");
      return [
        {
          file: file || top,
          anchor,
          title,
          // Index nodes list entries with links to elsewhere: nothing to read.
          index: /\brel="index"/.test(match[1] ?? "") || title.length < 50 && /(?:^|\s)Index$/i.test(title)
        }
      ];
    });
  };
  var chapterList = async (manual) => {
    const { top } = placeOf(manual);
    const node = await cachedPage(pageUrl(manual, top));
    freeLicence(node);
    const places = /* @__PURE__ */ new Set();
    const entries = tocEntries(node.text, top).filter(
      (entry) => !entry.index && PAGE.test(entry.file) && ANCHOR.test(entry.anchor) && entry.anchor !== "Top" && entry.anchor !== "SEC_Contents" && // A place listed twice (a chapter and its first section) is read once.
      !places.has(`${entry.file}#${entry.anchor}`) && Boolean(places.add(`${entry.file}#${entry.anchor}`))
    );
    const chapters = [];
    const seen = /* @__PURE__ */ new Set();
    entries.forEach((entry, index) => {
      const next = entries[index + 1];
      const end = next && next.file === entry.file && next.anchor ? next.anchor : "";
      const id = `${manual.id}/${entry.file}#${entry.anchor}#${end}`;
      if (seen.has(id)) return;
      seen.add(id);
      chapters.push({ id, name: entry.title || entry.file, number: chapters.length + 1 });
    });
    if (chapters.length === 0) throw new Error("This manual has no table of contents to read by");
    return chapters;
  };
  var anchorAt = (html, anchor) => {
    const found = [` id="${anchor}"`, ` name="${anchor}"`].map((attribute) => html.indexOf(attribute)).filter((at) => at >= 0);
    return found.length > 0 ? html.lastIndexOf("<", Math.min(...found)) : -1;
  };
  var cleanNode = (html) => dropElements(
    html.replace(/<!--[\s\S]*?-->/g, ""),
    (tag, attributes) => classes(attributes).some((name) => DROPPED_CLASSES.has(name))
  ).replace(/<hr\b[^>]*>/gi, "");
  var passage = async (chapterId) => {
    const match = /^([^/#]+)\/([^/#]+)#([^#]*)#([^#]*)$/.exec(chapterId);
    const [, id = "", file = "", start = "", end = ""] = match ?? [];
    if (!match || !PAGE.test(file) || !ANCHOR.test(start) || !ANCHOR.test(end)) {
      throw new Error("Not a chapter of a GNU manual");
    }
    const manual = manualOf(id);
    const url = pageUrl(manual, file);
    const found = await cachedPage(url);
    const body = /<body[^>]*>([\s\S]*?)<\/body>/i.exec(found.text)?.[1] ?? found.text;
    const from = start ? Math.max(anchorAt(body, start), 0) : 0;
    const until = end ? anchorAt(body, end) : -1;
    const html = cleanNode(body.slice(from, until > from ? until : body.length));
    if (!plainText(html)) throw new Error("This part of the manual has no text");
    return { html, baseUrl: found.url || url };
  };
  var summary = (manual) => ({ id: manual.id, title: manual.title });
  var pageOf = (manuals, page) => {
    const start = (Math.max(1, page) - 1) * PAGE_SIZE;
    return {
      items: manuals.slice(start, start + PAGE_SIZE).map(summary),
      hasNextPage: start + PAGE_SIZE < manuals.length
    };
  };
  var pick = (filters2, key, options) => {
    const value = filters2?.[key];
    return typeof value === "string" && options.some((option) => option.value === value) ? value : null;
  };
  var score = (manual, terms) => {
    const title = words(`${manual.title} ${manual.name}`);
    const about = words(`${manual.description} ${manual.category}`);
    let total = 0;
    for (const term of terms) {
      const found = (title.includes(term) ? 3 : 0) + (about.includes(term) ? 1 : 0);
      if (found === 0) return 0;
      total += found + (title.split(" ").includes(term) ? 1 : 0);
    }
    return total;
  };
  var search = (query, page, values) => {
    const category = pick(values, "category", CATEGORIES);
    const sort = pick(values, "sort", SORTS);
    const terms = words(query).split(" ").filter(Boolean);
    const listed = MANUALS.filter(
      (manual) => !category || category === "all" || manual.category === category
    );
    const found = terms.length ? listed.map((manual, order) => ({ manual, order, score: score(manual, terms) })).filter((match) => match.score > 0).sort((first, second) => second.score - first.score || first.order - second.order).map((match) => match.manual) : listed;
    const sorted = sort === "title" ? [...found].sort((first, second) => first.title.localeCompare(second.title)) : found;
    return pageOf(sorted, page);
  };
  var details = async (id) => {
    const manual = manualOf(id);
    const top = await Promise.race([
      cachedPage(pageUrl(manual, placeOf(manual).top)).catch(() => null),
      raygun.time.sleep(DETAILS_WAIT_MS).then(() => null)
    ]);
    const licence = top ? freeLicence(top) : manual.licence;
    return {
      id: manual.id,
      title: manual.title,
      coverUrl: null,
      authors: [],
      description: [
        manual.description,
        top ? introOf(top.text) : "",
        `Licence: ${licence}, as the manual's copying notice states.`
      ].filter(Boolean).join("\n\n"),
      genres: [manual.category],
      status: "completed",
      url: `${SITE}${manual.path}`
    };
  };
  var filters = [
    { kind: "select", key: "category", label: "Category", options: CATEGORIES },
    { kind: "select", key: "sort", label: "Sort", options: SORTS }
  ];
  registerProvider(
    defineProvider({
      kind: "novel.source",
      id: "manuals",
      name: "GNU Manuals",
      popular: async ({ page }) => pageOf(MANUALS, page),
      search: async ({ query, page, filters: values }) => search(query, page, values),
      filters: async () => filters,
      details: ({ id }) => details(id),
      chapters: async ({ id }) => ({ chapters: await chapterList(manualOf(id)) }),
      passage: ({ chapterId }) => passage(chapterId)
    })
  );
})();
