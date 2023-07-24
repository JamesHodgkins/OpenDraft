#ifndef OD_GR_GRAPHICS_H
#define OD_GR_GRAPHICS_H

/**************************************************************************************
* OpenDraft:    Main Graphics Header                                                  *
*-------------------------------------------------------------------------------------*
* Filename:     OdGraphics.h                                                          *
* Contributors: James Hodgkins                                                        *
* Date:         June 26, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Header file collates all necessary graphics                                       *
***************************************************************************************/



#define GLEW_STATIC					// Use static glew library (glew32s.lib)
#include "GL/glew.h"				// Include glew
#include "glfw/glfw3.h"				// Include glfw3
#include "nanovg.h"					// Include core nanovg library

#include "System/OdCore.h"
#include "Graphics/OdComponents.h"
#include "Graphics/OdDraw.h"

namespace OD
{
	namespace Graphics
	{

		class OdGraphics
		{

		};

	} // namespace Graphics
}// namespace OD

#endif // !OD_GR_GRAPHICS_H