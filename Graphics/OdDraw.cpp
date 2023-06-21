#ifndef OD_GR_DRAW_CPP
#define OD_GR_DRAW_CPP

/**************************************************************************************
* OpenDraft:    Graphic Draw Class                                                    *
*-------------------------------------------------------------------------------------*
* Filename:     OdDraw.cpp                                                            *
* Contributors: James Hodgkins                                                        *
* Date:         June 21, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Draw routine wrappers for NanoVG intended for GUI rendering                       *
***************************************************************************************/



#include "OdDraw.h"

namespace OD
{
	namespace Graphics
	{
		void OdDraw::Rect(NVGcontext* aContext, int aX, int aY, int aWidth, int aHeight, const OdColour aColour)
		{
			nvgBeginPath(aContext);
			nvgRect(aContext, aX, aY, aWidth, aHeight);
			nvgFillColor(aContext, aColour.asNvgColour());
			nvgFill(aContext);
			nvgClosePath(aContext);
		}

		void OdDraw::RectStroke(NVGcontext* aContext, int aX, int aY, int aWidth, int aHeight, const OdColour aColour)
		{
			nvgBeginPath(aContext);
			nvgRect(aContext, aX, aY, aWidth, aHeight);
			nvgStrokeColor(aContext, aColour.asNvgColour());
			nvgStroke(aContext);
			nvgClosePath(aContext);
		}

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

		void OdDraw::ResourceImage(NVGcontext* aContext, int aX, int aY, int aWidth, int aHeight, OdImage* aImage)
		{
			if (aImage == nullptr)
				return;

			if (aImage->getId() != 0)
			{
				nvgBeginPath(aContext);
				nvgRect(aContext, aX, aY, aWidth, aHeight);
				nvgFillPaint(aContext, nvgImagePattern(aContext, aX, aY, aWidth, aHeight, 0, aImage->getId(), 1.0f));
				nvgFill(aContext);
				nvgClosePath(aContext);
			}
		}

		void OdDraw::ResourceImage(NVGcontext* aContext, int aX, int aY, OdImage* aImage)
		{
			if (aImage == nullptr)
				return;

			OdDraw::ResourceImage(aContext, aX, aY, aImage->getWidth(), aImage->getHeight(), aImage);
		}

	} // namespace Graphics
} // namespace OD

#endif // OD_GR_DRAW_CPP