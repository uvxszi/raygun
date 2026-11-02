/* Raygun's LAME configuration: one header for every target, picked by the target rather than the
 * machine building it. Upstream's build used configMS.h on Windows hosts, whose int macros break
 * Android's <stdint.h> when cross-compiling. */
#ifndef RAYGUN_LAME_CONFIG_H
#define RAYGUN_LAME_CONFIG_H

#define STDC_HEADERS 1
#define HAVE_ERRNO_H 1
#define HAVE_FCNTL_H 1
#define HAVE_LIMITS_H 1
#define HAVE_STDINT_H 1
#define HAVE_STRCHR 1
#define HAVE_MEMCPY 1
#define PROTOTYPES 1
#define USE_FAST_LOG 1
#define PACKAGE "lame"
#define LAME_LIBRARY_BUILD

#include <stdint.h>

typedef long double ieee854_float80_t;
typedef double      ieee754_float64_t;
typedef float       ieee754_float32_t;

/* The SSE paths, as upstream enables them for 64-bit MSVC. */
#if defined(_M_X64) && !defined(HAVE_XMMINTRIN_H)
#define HAVE_XMMINTRIN_H
#endif

#endif
