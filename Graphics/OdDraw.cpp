#ifndef OD_GR_DRAW_CPP
#define OD_GR_DRAW_CPP

/**************************************************************************************
* OpenDraft:    Graphic Draw Class                                                    *
*-------------------------------------------------------------------------------------*
* Filename:     OdDraw.cpp                                                            *
* Contributors: James Hodgkins                                                        *
* Date:         June 21, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Draw routine wrappers for NanoVG intended for GUI rendering                       *
***************************************************************************************/



#include "OdDraw.h"

namespace OD::Graphics
{
	void OdDraw::Line(NVGcontext* aContext, float aX1, float aY1, float aX2, float aY2, float thickness, OdColour aColour)
	{
		nvgBeginPath(aContext);
		nvgMoveTo(aContext, aX1, aY1);
		nvgLineTo(aContext, aX2, aY2);
		nvgStrokeWidth(aContext, thickness);
		nvgStrokeColor(aContext, aColour.asNvgColour());
		nvgStroke(aContext);
		nvgClosePath(aContext);
	}

	void OdDraw::Rect(NVGcontext* aContext, float aX, float aY, float aWidth, float aHeight, const OdColour aColour)
	{
		nvgBeginPath(aContext);
		nvgMoveTo(aContext, aX, aY);
		nvgRect(aContext, aX, aY, aWidth, aHeight);
		nvgFillColor(aContext, aColour.asNvgColour());
		nvgFill(aContext);
		nvgClosePath(aContext);
	}

	void OdDraw::RectStroke(NVGcontext* aContext, float aX, float aY, float aWidth, float aHeight, float aWeight, const OdColour aColour)
	{
		nvgBeginPath(aContext);
		nvgRect(aContext, aX, aY, aWidth, aHeight);
		nvgStrokeColor(aContext, aColour.asNvgColour());
		nvgStrokeWidth(aContext, aWeight);
		nvgStroke(aContext);
		nvgClosePath(aContext);
	}

	void OdDraw::Text(NVGcontext* aContext, float aX, float aY, float aWidth, float aHeight, const TextStyle* aStyle, const char* text)
	{
		nvgBeginPath(aContext);
		nvgFillColor(aContext, aStyle->colour.asNvgColour());
		nvgFill(aContext);
		nvgFontSize(aContext, aStyle->size);
		nvgFontFace(aContext, aStyle->font);

		// Set text alignment - TODO: Add support for vertical alignment
		if (aStyle->align.getAlign() & OdAlign::LEFT)
		{
			nvgTextAlign(aContext, NVG_ALIGN_LEFT | NVG_ALIGN_MIDDLE);
			nvgText(aContext, aX, aY + aHeight / 2, text, nullptr);
		}
		else if (aStyle->align.getAlign() & OdAlign::RIGHT)
		{
			nvgTextAlign(aContext, NVG_ALIGN_RIGHT | NVG_ALIGN_MIDDLE);
			nvgText(aContext, aX + aWidth / 2, aY + aHeight / 2, text, nullptr);
		}
		else
		{
			nvgTextAlign(aContext, NVG_ALIGN_CENTER | NVG_ALIGN_MIDDLE);
			nvgText(aContext, aX + aWidth / 2, aY + aHeight / 2, text, nullptr);
		}
		
		nvgClosePath(aContext);
	}

	void OdDraw::ResourceImage(NVGcontext* aContext, float aX, float aY, float aWidth, float aHeight, OdImage* aImage)
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
		}
	}

	void OdDraw::ResourceImage(NVGcontext* aContext, float aX, float aY, OdImage* aImage)
	{
		// Check if image is valid
		if (aImage == nullptr)
			return;
		
		OdDraw::ResourceImage(aContext, aX, aY, aImage->getWidth(), aImage->getHeight(), aImage);
	}

	void OdDraw::Translate(NVGcontext* aContext, float aX, float aY)
	{
		nvgTranslate(aContext, aX, aY);
	}

	void OdDraw::Scale(NVGcontext* aContext, float aX, float aY)
	{
		nvgScale(aContext, aX, aY);
	}

	void OdDraw::Rotate(NVGcontext* aContext, float aAngle)
	{
		nvgRotate(aContext, aAngle);
	}

	void OdDraw::ResetTransform(NVGcontext* aContext)
	{
		nvgResetTransform(aContext);
	}

} // namespace OD::Graphics

#endif // !OD_GR_DRAW_CPP