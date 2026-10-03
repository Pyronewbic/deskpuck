#pragma once

#import "JMConfig.h"

#include "Joycon2Engine.h"

// Engine settings for a config. Only call with a config whose
// validationProblems is empty.
EngineSettings JMEngineSettingsFromConfig(JMConfig* config);
