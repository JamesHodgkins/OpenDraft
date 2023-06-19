#pragma once

/**************************************************************************************
* OpenDraft:    Graphic Draw Class                                                    *
*-------------------------------------------------------------------------------------*
* Filename:     OdGrDraw.h                                                            *
* Contributors: James Hodgkins                                                        *
* Date:         June 15, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Draw routine wrappers for NanoVG intended for GUI rendering                       *
***************************************************************************************/



#include "Libraries/nanovg/src/nanovg.h"
#include "System/OdSyCore.h"
#include <iostream>



class OdGrDraw
{
public:

	// Draws a filled rectangle on the specified NanoVG aContext.
	static void Rect(NVGcontext* aContext, int aX, int aY, int aWidth, int aHeight, OdSyColour aColour);

	// Draws a stroked rectangle on the specified NanoVG aContext.
	static void RectStroke(NVGcontext* aContext, int aX, int aY, int aWidth, int aHeight, OdSyColour aColour);

	// Draws text on the specified NanoVG aContext.
	static void Text(NVGcontext* aContext, int aX, int aY, int aWidth, int aHeight, float aSize, OdSyColour aColour, const char* aText);

	// Draws an image on the specified NanoVG aContext from a file path.
	static void ResourceImage(NVGcontext* aContext, int aX, int aY, int aWidth, int aHeight, OdSyImage* aImage);
	static void ResourceImage(NVGcontext* aContext, int aX, int aY, OdSyImage* aImage);

};




void OdGrDraw::Rect(NVGcontext* aContext, int aX, int aY, int aWidth, int aHeight, OdSyColour aColour)
{
	nvgBeginPath(aContext);
	nvgRect(aContext, aX, aY, aWidth, aHeight);
	nvgFillColor(aContext, aColour.asNvgColour());
	nvgFill(aContext);
	nvgClosePath(aContext);
}


void OdGrDraw::RectStroke(NVGcontext* aContext, int aX, int aY, int aWidth, int aHeight, OdSyColour aColour)
{
	nvgBeginPath(aContext);
	nvgRect(aContext, aX, aY, aWidth, aHeight);
	nvgStrokeColor(aContext, aColour.asNvgColour());
	nvgStroke(aContext);
	nvgClosePath(aContext);
}


void OdGrDraw::Text(NVGcontext* aContext, int aX, int aY, int aWidth, int aHeight, float aSize, OdSyColour aColour, const char* text)
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


void OdGrDraw::ResourceImage(NVGcontext* aContext, int aX, int aY, int aWidth, int aHeight, OdSyImage* aImage)
{
	if (aImage == nullptr)
		return;

	if (aImage->getId() != 0)
	{
		nvgBeginPath(aContext);
		nvgRect(aContext, aX, aY, aImage->getWidth(), aImage->getHeight());
		nvgFillPaint(aContext, nvgImagePattern(aContext, aX, aY, aWidth, aHeight, 0, aImage->getId(), 1.0f));
		nvgFill(aContext);
		nvgClosePath(aContext);
		//nvgDeleteImage(aContext, imageHandle); // To do link delete image mem to deleting image class
	}
}

void OdGrDraw::ResourceImage(NVGcontext* aContext, int aX, int aY, OdSyImage* aImage)
{
	if (aImage == nullptr)
		return;

	OdGrDraw::ResourceImage(aContext, aX, aY, aImage->getWidth(), aImage->getHeight(), aImage);
}