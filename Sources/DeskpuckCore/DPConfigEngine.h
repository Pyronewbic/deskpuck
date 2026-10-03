#pragma once

#import "DPConfig.h"

#include "Joycon2Engine.h"

// Engine settings for a config. Only call with a config whose
// validationProblems is empty.
EngineSettings DPEngineSettingsFromConfig(DPConfig* config);
