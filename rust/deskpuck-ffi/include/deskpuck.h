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
#define DP_ABI_VERSION 4

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
    // Nothing paired and no pairing window open: nothing will connect.
    DP_STATUS_NOT_PAIRED = 6,
    // A pairing window is open: the first Joy-Con found becomes the paired one.
    DP_STATUS_PAIRING = 7,
    // The paired Joy-Con is connected to this Mac by another program.
    DP_STATUS_IN_USE_ELSEWHERE = 8,
} dp_status;

// How long dp_controller_start_pairing accepts a new Joy-Con.
#define DP_PAIRING_SECONDS 60

// Called on a background thread. device_name may be NULL and is only valid
// during the call.
typedef void (*dp_status_callback)(void *context, dp_status status, const char *device_name);

// Bits of the modifiers latched on by modifier buttons.
#define DP_MODIFIER_CONTROL 1
#define DP_MODIFIER_OPTION 2
#define DP_MODIFIER_SHIFT 4
#define DP_MODIFIER_COMMAND 8

// Called on a background thread whenever the latched modifiers change.
typedef void (*dp_latch_callback)(void *context, uint32_t modifiers);

typedef struct dp_controller dp_controller;

// Starts looking for the Joy-Con paired in pairing.json beside the default
// config.json; with none, reports DP_STATUS_NOT_PAIRED. NULL if config_json is invalid or the
// background thread could not start. Runs before Accessibility is granted;
// input is dropped by macOS until it is.
dp_controller *dp_controller_start(const char *config_json, dp_status_callback on_status,
                                   dp_latch_callback on_latch, void *context);
void dp_controller_set_paused(dp_controller *controller, bool paused);
bool dp_controller_is_paused(const dp_controller *controller);
// Opens a pairing window, disconnecting the current Joy-Con. The first
// Joy-Con 2 that connects replaces the saved pairing. Ignored while paused.
void dp_controller_start_pairing(dp_controller *controller);
// Closes the window early; the paired Joy-Con, if any, is searched for again.
void dp_controller_cancel_pairing(dp_controller *controller);
// Applies new settings live. NULL on success, else the problems.
char *dp_controller_apply_config(dp_controller *controller, const char *config_json);
// Releases held input, disconnects, and waits for the background thread:
// on_status is never called after this returns. NULL is ignored.
void dp_controller_free(dp_controller *controller);

#ifdef __cplusplus
}
#endif

#endif
