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

			// Draws a filled rectangle on the specified NanoVG aContext.
			static void Rect(NVGcontext* aContext, int aX, int aY, int aWidth, int aHeight, OdColour aColour);

			// Draws a stroked rectangle on the specified NanoVG aContext.
			static void RectStroke(NVGcontext* aContext, int aX, int aY, int aWidth, int aHeight, OdColour aColour);

			// Draws text on the specified NanoVG aContext.
			static void Text(NVGcontext* aContext, int aX, int aY, int aWidth, int aHeight, float aSize, OdColour aColour, const char* aText);

			// Draws an image on the specified NanoVG aContext from a file path.
			static void ResourceImage(NVGcontext* aContext, int aX, int aY, int aWidth, int aHeight, OdImage* aImage);
			static void ResourceImage(NVGcontext* aContext, int aX, int aY, OdImage* aImage);

		};



		// Draws a filled rectangle on the specified NanoVG aContext.
		void OdDraw::Rect(NVGcontext* aContext, int aX, int aY, int aWidth, int aHeight, const OdColour aColour)
		{
			nvgBeginPath(aContext);
			nvgRect(aContext, aX, aY, aWidth, aHeight);
			nvgFillColor(aContext, aColour.asNvgColour());
			nvgFill(aContext);
			nvgClosePath(aContext);
		}


		// Draws a stroked rectangle on the specified NanoVG aContext.
		void OdDraw::RectStroke(NVGcontext* aContext, int aX, int aY, int aWidth, int aHeight, const OdColour aColour)
		{
			nvgBeginPath(aContext);
			nvgRect(aContext, aX, aY, aWidth, aHeight);
			nvgStrokeColor(aContext, aColour.asNvgColour());
			nvgStroke(aContext);
			nvgClosePath(aContext);
		}


		// Draws text on the specified NanoVG aContext.
		void OdDraw::Text(NVGcontext* aContext, int aX, int aY, int aWidth, int aHeight, float aSize, OdColour aColour, const char* text)
		{
			nvgBeginPath(aContext);
			nvgFillColor(aContext, aColour.asNvgColour());
			nvgFill(aContext);
			nvgFontSize(aContext, aSize);
			nvgFontFace(aContext, "sans");
			nvgTextAlign(aContext, NVG_ALIGN_CENTER | NVG_ALIGN_MIDDLE);
			nvgText(aContext, aX + aWidth / 2, aY + aHeight / 2, text, nullptr);
			nvgClosePath(aContext);
		}


		// Draws an image on the specified NanoVG aContext from a file path.
		void OdDraw::ResourceImage(NVGcontext* aContext, int aX, int aY, int aWidth, int aHeight, OdImage* aImage)
		{
			// Check if image is valid
			if (aImage == nullptr)
				return;

			if (aImage->getId() != 0)
			{
				nvgBeginPath(aContext);
				nvgRect(aContext, aX, aY, aWidth, aHeight);
				nvgFillPaint(aContext, nvgImagePattern(aContext, aX, aY, aWidth, aHeight, 0, aImage->getId(), 1.0f));
				nvgFill(aContext);
				nvgClosePath(aContext);
				//nvgDeleteImage(aContext, imageHandle); // To do link delete image mem to deleting image class
			}
		}


		// Draws an image on the specified NanoVG aContext from a file path.
		void OdDraw::ResourceImage(NVGcontext* aContext, int aX, int aY, OdImage* aImage)
		{
			// Check if image is valid
			if (aImage == nullptr)
				return;

			OdDraw::ResourceImage(aContext, aX, aY, aImage->getWidth(), aImage->getHeight(), aImage);
		}

	} // namespace Graphics
}// namespace OD


#endif // OD_GR_DRAW_H