/*
 * libretro's retro_log_printf_t is variadic (printf-style). Stable Rust cannot
 * define variadic functions, and reading only the named args is unsound on
 * Apple arm64 (variadics go on the stack). This shim formats the message with
 * vsnprintf and hands a plain C string to a Rust sink.
 */
#include <stdarg.h>
#include <stdio.h>

typedef void (*nostyra_log_sink_t)(unsigned level, const char *message);

static nostyra_log_sink_t g_sink = NULL;

/* Set once before the core receives the log interface; emulation thread only. */
void nostyra_set_log_sink(nostyra_log_sink_t sink) { g_sink = sink; }

/* Handed to the core as retro_log_callback.log. Stack buffer: no allocation. */
void nostyra_log_printf(unsigned level, const char *fmt, ...)
{
    char buf[1024];
    va_list ap;
    if (!g_sink || !fmt)
        return;
    va_start(ap, fmt);
    vsnprintf(buf, sizeof buf, fmt, ap);
    va_end(ap);
    g_sink(level, buf);
}
