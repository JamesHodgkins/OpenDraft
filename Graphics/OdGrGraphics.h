#ifndef OD_GR_GRAPHICS_H
#define OD_GR_GRAPHICS_H

#define GLEW_STATIC					// Use static glew library (glew32s.lib)
#include "GL/glew.h"				// Include glew
#include "glfw/glfw3.h"				// Include glfw3

#define NANOVG_GL3_IMPLEMENTATION	// Use GL3.
#include "nanovg.h"					// Include core nanovg library
#include "nanovg_gl.h"				// Include nanovg opengl3 implementation

#include "System/OdSyCore.h"
#include "Graphics/OdGrUiComponents.h"
#include "Graphics/OdGrDraw.h"

class GrGraphics
{

};

#endif // OD_GR_GRAPHICS_H