// C interface to the Deskpuck core. Strings are UTF-8 and NUL-terminated.
// Every returned char* is owned by the caller and freed with dp_string_free.
// No function unwinds: a Rust panic becomes an error return.

#ifndef DESKPUCK_H
#define DESKPUCK_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// Bumped on any change to this header; the app checks it at startup so a
// stale static library fails loudly instead of misbehaving.
#define DP_ABI_VERSION 1

uint32_t dp_abi_version(void);
void dp_string_free(char *string);

// ---- Settings (config.json) ----

// The per-user config file path, or NULL if there is no home directory.
char *dp_config_default_path(void);
// The default settings as config JSON.
char *dp_config_defaults(void);
// {"config": <config JSON>, "warnings": [<string>...]}. A missing file gives
// the defaults with no warnings; anything unusable falls back to its default.
char *dp_config_load(const char *path);
// A JSON array of problems; [] means the config is valid.
char *dp_config_problems(const char *config_json);
// Validates, then writes atomically. NULL on success, else a message.
char *dp_config_save(const char *config_json, const char *path);

// ---- Controller ----

typedef enum {
    DP_STATUS_BLUETOOTH_OFF = 0,
    DP_STATUS_BLUETOOTH_UNAUTHORIZED = 1,
    DP_STATUS_UNAVAILABLE = 2,
    DP_STATUS_SEARCHING = 3,
    DP_STATUS_CONNECTING = 4,
    DP_STATUS_CONNECTED = 5,
} dp_status;

// Called on a background thread. device_name may be NULL and is only valid
// during the call.
typedef void (*dp_status_callback)(void *context, dp_status status, const char *device_name);

typedef struct dp_controller dp_controller;

// Starts looking for a Joy-Con. NULL if config_json is invalid or the
// background thread could not start. Runs before Accessibility is granted;
// input is dropped by macOS until it is.
dp_controller *dp_controller_start(const char *config_json, dp_status_callback on_status, void *context);
void dp_controller_set_paused(dp_controller *controller, bool paused);
bool dp_controller_is_paused(const dp_controller *controller);
// Applies new settings live. NULL on success, else the problems.
char *dp_controller_apply_config(dp_controller *controller, const char *config_json);
// Releases held input, disconnects, and waits for the background thread:
// on_status is never called after this returns. NULL is ignored.
void dp_controller_free(dp_controller *controller);

#ifdef __cplusplus
}
#endif

#endif
