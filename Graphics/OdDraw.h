#ifndef OD_GR_DRAW_H
#define OD_GR_DRAW_H

/**************************************************************************************
* OpenDraft:    Graphic Draw Class                                                    *
*-------------------------------------------------------------------------------------*
* Filename:     OdDraw.h                                                            *
* Contributors: James Hodgkins                                                        *
* Date:         June 15, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Draw routine wrappers for NanoVG intended for GUI rendering                       *
***************************************************************************************/



#include "Libraries/nanovg/src/nanovg.h"
#include "System/OdCore.h"
#include <iostream>

using namespace OD::System;

namespace OD
{
	namespace Graphics
	{
		class OdDraw
		{
		public:
			static void Rect(NVGcontext* aContext, int aX, int aY, int aWidth, int aHeight, OdColour aColour);
			static void RectStroke(NVGcontext* aContext, int aX, int aY, int aWidth, int aHeight, OdColour aColour);
			static void Text(NVGcontext* aContext, int aX, int aY, int aWidth, int aHeight, float aSize, OdColour aColour, const char* aText);
			static void ResourceImage(NVGcontext* aContext, int aX, int aY, int aWidth, int aHeight, OdImage* aImage);
			static void ResourceImage(NVGcontext* aContext, int aX, int aY, OdImage* aImage);
		};
	} // namespace Graphics
} // namespace OD

#endif // OD_GR_DRAW_H