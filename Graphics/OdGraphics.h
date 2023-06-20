#ifndef OD_GR_GRAPHICS_H
#define OD_GR_GRAPHICS_H

#define GLEW_STATIC					// Use static glew library (glew32s.lib)
#include "GL/glew.h"				// Include glew
#include "glfw/glfw3.h"				// Include glfw3

#define NANOVG_GL3_IMPLEMENTATION	// Use GL3.
#include "nanovg.h"					// Include core nanovg library
#include "nanovg_gl.h"				// Include nanovg opengl3 implementation

#include "System/OdCore.h"
#include "Graphics/OdComponents.h"
#include "Graphics/OdDraw.h"

namespace OD
{
	namespace Graphics
	{

		class GrGraphics
		{

		};

	} // namespace Graphics
}// namespace OD

#endif // OD_GR_GRAPHICS_H