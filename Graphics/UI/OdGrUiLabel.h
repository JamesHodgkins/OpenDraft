#pragma once

/**************************************************************************************
* OpenDraft:    GUI Label Class                                                       *
*-------------------------------------------------------------------------------------*
* Filename:     OdGrUiLabel.h                                                         *
* Contributors: James Hodgkins                                                        *
* Date:         June 13, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A GUI label class derived by OdGrComponent                                        *
***************************************************************************************/



#include "System/OdSyCore.h"
#include "Graphics/UI/OdGrUiComponent.h"
#include "Graphics/OdGrDraw.h"


class OdGrUiLabel : public OdGrUiComponent
{
protected:

public:

	/**
	 * \brief Constructs a Label object with the specified attributes.
	 * \param location (OdSyPoint) The position of the Label.
	 * \param size (OdSyPoint) The size of the Label.
	 * \param text (std::string) The text displayed on the Label.
	 */
	OdGrUiLabel(int aX = 0, int aY = 0, int aWidth = 150, int aHeight = 30, std::string aText = "Label")
	{
		location.x = aX;
		location.y = aY;
		size.x = aWidth;
		size.y = aHeight;

		backColour = OdSyColour(255, 0, 0, 255);
		foreColour = OdSyColour(255, 255, 255, 200);

		text = aText;
	}


	/**
	 * \brief Renders a default OD-GUI Label to a given NanoVG context (NVGContext) with the specified attributes.
	 * \param context (NVGcontext*) The nanovg pointer for rendering.
	 */
	virtual void const onFrame(NVGcontext* aContext) override
	{
		if (aContext == nullptr)
			return;

		// Convert class location (relative to parent) to context location (relative to the window)
		OdSyPoint baseLocation;

		if (parent == nullptr) {
			baseLocation = OdSyPoint(location.x, location.y);
		}
		else
		{
			baseLocation = OdSyPoint(parent->getLocationInContext().x + location.x, parent->getLocationInContext().y + location.y);
		}

		//
		// Begin drawing Label
		//
		OdGrDraw::Rect(aContext, baseLocation.x, baseLocation.y, size.x, size.y, backColour);
		OdGrDraw::Text(aContext, baseLocation.x, baseLocation.y, size.x, size.y, 14.0f, foreColour, text.c_str());


	}

};