// Every returned char* is owned by the caller and freed with dp_string_free.
// No function unwinds: a Rust panic becomes an error return.

#ifndef DESKPUCK_H
#define DESKPUCK_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// Bumped on any change to this header; the app checks it at startup.
#define DP_ABI_VERSION 8

uint32_t dp_abi_version(void);
void dp_string_free(char *string);

// NULL if there is no home directory.
char *dp_config_default_path(void);
char *dp_config_defaults(void);
// {"config": <config JSON>, "warnings": [<string>...]}. A missing file gives
// the defaults with no warnings; anything unusable falls back to its default.
char *dp_config_load(const char *path);
// A JSON array of problems; [] means the config is valid.
char *dp_config_problems(const char *config_json);
// NULL on success, else a message.
char *dp_config_save(const char *config_json, const char *path);

// Fetched without following its redirect, and the page to open for an update.
#define DP_LATEST_RELEASE_URL "https://github.com/Pyronewbic/deskpuck/releases/latest"
// Any non-empty value turns the update check off.
#define DP_UPDATE_DISABLE_ENV "DESKPUCK_NO_UPDATE_CHECK"
// location: the Location header of the latest-release redirect; current: the
// running version. The newer version ("0.4.0"), or NULL for anything else.
char *dp_update_evaluate(const char *location, const char *current);
// Whether the settings file allows the check now: a missing file does (the
// default), one that cannot be read does not.
bool dp_update_allowed(const char *config_path);

// One-click update of the Mac app. version: what dp_update_evaluate returned;
// current: the running version. For a strictly newer version, the files to fetch
// into one folder: {"version", "package": <file name>, "downloads": [{"name",
// "url", "maxBytes"}...]}; else NULL.
char *dp_release_plan(const char *version, const char *current);
// Whether a download may follow a redirect to url.
bool dp_release_redirect_allowed(const char *url);
// NULL once dir's SHA256SUMS is signed with the release key and lists the
// package with a matching hash; else why not. Install nothing on a message.
char *dp_release_verify(const char *dir, const char *version, const char *current);

// The settings window, in points, sized and placed as on Linux and Windows.
// Its inner height for content between a header and a footer, up to 0.85 of
// screen_height; a screen_height that is not positive and finite caps nothing.
double dp_window_height(double content, double header, double footer, double screen_height);
// Whether to move a window whose top is top below the screen's and which is
// height tall: centred when first, else only lifted to keep its bottom on the
// screen. On true, *new_top (unless NULL) is where its top goes.
bool dp_window_top(bool first, double top, double height, double screen_height, double *new_top);

typedef enum {
    DP_STATUS_BLUETOOTH_OFF = 0,
    DP_STATUS_BLUETOOTH_UNAUTHORIZED = 1,
    DP_STATUS_UNAVAILABLE = 2,
    DP_STATUS_SEARCHING = 3,
    DP_STATUS_CONNECTING = 4,
    DP_STATUS_CONNECTED = 5,
    DP_STATUS_NOT_PAIRED = 6,
    DP_STATUS_PAIRING = 7,
    DP_STATUS_IN_USE_ELSEWHERE = 8,
} dp_status;

#define DP_PAIRING_SECONDS 60

// Called on a background thread. device_name may be NULL and is only valid
// during the call.
typedef void (*dp_status_callback)(void *context, dp_status status, const char *device_name);

#define DP_MODIFIER_CONTROL 1
#define DP_MODIFIER_OPTION 2
#define DP_MODIFIER_SHIFT 4
#define DP_MODIFIER_COMMAND 8

// Called on a background thread.
typedef void (*dp_latch_callback)(void *context, uint32_t modifiers);

typedef struct dp_controller dp_controller;

// NULL if config_json is invalid or the thread could not start. Runs before
// Accessibility is granted; macOS drops input until it is.
dp_controller *dp_controller_start(const char *config_json, dp_status_callback on_status,
                                   dp_latch_callback on_latch, void *context);
void dp_controller_set_paused(dp_controller *controller, bool paused);
bool dp_controller_is_paused(const dp_controller *controller);
// Disconnects the current Joy-Con; the next one to connect replaces the saved
// pairing. Ignored while paused. host_address: this Mac's Bluetooth address
// ("AA:BB:CC:DD:EE:FF"), or NULL; with buttonReconnect on, the Joy-Con learns
// it and reconnects on a button press. Anything else, all zeros included, is
// unknown and pairs for SYNC only.
void dp_controller_start_pairing(dp_controller *controller, const char *host_address);
void dp_controller_cancel_pairing(dp_controller *controller);
// NULL on success, else the problems.
char *dp_controller_apply_config(dp_controller *controller, const char *config_json);
// Waits for the background thread: on_status is never called after this
// returns. NULL is ignored.
void dp_controller_free(dp_controller *controller);

#ifdef __cplusplus
}
#endif

#endif
