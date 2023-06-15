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

	// Draws a filled rectangle on the specified NanoVG context.
	static void Rect(NVGcontext* vg, int x, int y, int width, int height, OdSyColour colour);

	// Draws a stroked rectangle on the specified NanoVG context.
	static void RectStroke(NVGcontext* vg, int x, int y, int width, int height, OdSyColour colour);

	// Draws text on the specified NanoVG context.
	static void Text(NVGcontext* vg, int x, int y, int width, int height, OdSyColour colour, const char* text);

	static void LoadImage(NVGcontext* vg, int width, int height, const char* filePath, OdSyImage** img);

	// Draws an image on the specified NanoVG context from a file path.
	static void ImageFromPath(NVGcontext* vg, int x, int y, OdSyImage* img);

	// TEMP: Loads a font into the specified NanoVG context.
	static void LoadFont(NVGcontext* vg);
};



void OdGrDraw::LoadFont(NVGcontext* vg)
{
	nvgCreateFont(vg, "sans", ".\\OpenSans.ttf");
}


void OdGrDraw::Rect(NVGcontext* vg, int x, int y, int width, int height, OdSyColour colour)
{
	nvgBeginPath(vg);
	nvgRect(vg, x, y, width, height);
	nvgFillColor(vg, colour.asNvgColour());
	nvgFill(vg);
	nvgClosePath(vg);
}


void OdGrDraw::RectStroke(NVGcontext* vg, int x, int y, int width, int height, OdSyColour colour)
{
	nvgBeginPath(vg);
	nvgRect(vg, x, y, width, height);
	nvgStrokeColor(vg, colour.asNvgColour());
	nvgStroke(vg);
	nvgClosePath(vg);
}


void OdGrDraw::Text(NVGcontext* vg, int x, int y, int width, int height, OdSyColour colour, const char* text)
{
	nvgBeginPath(vg);
	nvgFillColor(vg, colour.asNvgColour());
	nvgFill(vg);
	nvgFontSize(vg, 14.0f);
	nvgFontFace(vg, "sans");
	nvgTextAlign(vg, NVG_ALIGN_CENTER | NVG_ALIGN_MIDDLE);
	nvgText(vg, x + width / 2, y + height / 2, text, nullptr);
	nvgClosePath(vg);
}


void OdGrDraw::LoadImage(NVGcontext* vg, int width, int height, const char* filePath, OdSyImage** img)
{
	int imageHandle = nvgCreateImage(vg, ".\\testimage.jpg", 0);
	*img = new OdSyImage(width, height, 1.0, filePath, imageHandle);
}


void OdGrDraw::ImageFromPath(NVGcontext* vg, int x, int y, OdSyImage* img)
{
	std::cout << img->getId() << std::endl;

	if (img->getId() != 0)
	{
		nvgBeginPath(vg);
		nvgRect(vg, x, y, img->getWidth(), img->getHeight());
		nvgFillPaint(vg, nvgImagePattern(vg, x, y, img->getWidth(), img->getHeight(), 0, img->getId(), 1.0f));
		nvgFill(vg);
		nvgClosePath(vg);
		//nvgDeleteImage(vg, imageHandle); // To do link delete image mem to deleting image class
	}
}