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
			static void Rect(NVGcontext* aContext, float aX, float aY, float aWidth, float aHeight, OdColour aColour);
			static void RectStroke(NVGcontext* aContext, float aX, float aY, float aWidth, float aHeight, OdColour aColour);
			static void Text(NVGcontext* aContext, float aX, float aY, float aWidth, float aHeight, float aSize, OdColour aColour, const char* aText);
			static void ResourceImage(NVGcontext* aContext, float aX, float aY, float aWidth, float aHeight, OdImage* aImage);
			static void ResourceImage(NVGcontext* aContext, float aX, float aY, OdImage* aImage);
			static void Translate(NVGcontext* aContext, float aX, float aY);
			static void Scale(NVGcontext* aContext, float aX, float aY);
			static void Rotate(NVGcontext* aContext, float aAngle);
			static void ResetTransform(NVGcontext* aContext);
		};

	} // OD::Graphics
} // namespace OD

#endif // OD_GR_DRAW_H