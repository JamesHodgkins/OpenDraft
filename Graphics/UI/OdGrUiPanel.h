#ifndef OD_GR_UI_PANEL_H
#define OD_GR_UI_PANEL_H

/**************************************************************************************
* OpenDraft:    GUI Panel Container Class                                             *
*-------------------------------------------------------------------------------------*
* Filename:     OdGrUiPanel.h                                                         *
* Contributors: James Hodgkins                                                        *
* Date:         June 18, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A GUI panel container class derived from OdGrComponent                            *
***************************************************************************************/



#include "System/OdSyCore.h"
#include "Graphics/UI/OdGrUiComponent.h"
#include "Graphics/OdGrDraw.h"


class OdGrUiPanel : public OdGrUiComponent
{
protected:
	OdSyColour backColourHover;
	OdSyColour backColourActive;
	OdSyImage* backgroundImage;

public:

	/**
	 * \brief Constructs a Panel object with the specified attributes.
	 * \param location (OdSyPoint) The position of the panel.
	 * \param size (OdSyPoint) The size of the panel.
	 * \param text (std::string) The text displayed on the panel.
	 */
	OdGrUiPanel(int aX = 0, int aY = 0, int aWidth = 150, int aHeight = 30)
	{
		location.x = aX;
		location.y = aY;
		size.x = aWidth;
		size.y = aHeight;

		backColour = OdSyColour::BACKGROUND1;
		backColourHover = OdSyColour::BACKGROUND2;
		backColourActive = OdSyColour(0, 0, 255, 255);
		stroke = OdSyColour(0, 0, 0, 0);
		foreColour = OdSyColour(255, 255, 255, 200);

		backgroundImage = nullptr;

		text = "";
	}

	void setBackgroundImage(OdSyImage* aImage)
	{
		backgroundImage = aImage;
	}

	void clearBackgroundImage()
	{
		backgroundImage = nullptr;
	}


	/**
	 * \brief Renders a default OD-GUI Panel to a given NanoVG context (NVGContext) with the specified attributes.
	 * \param context (NVGcontext*) The nanovg pointer for rendering.
	 */
	virtual void const onFrame(NVGcontext* aContext) override
	{
		if (!enabled)
			return;

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
		// Begin drawing Panel
		//
		OdGrDraw::Rect(aContext, baseLocation.x, baseLocation.y, size.x, size.y, backColour);
	


		if (backgroundImage != nullptr)
		{
			OdGrDraw::ResourceImage(aContext, baseLocation.x, baseLocation.y, size.x, size.y, backgroundImage);
		}

		OdGrDraw::RectStroke(aContext, baseLocation.x, baseLocation.y, size.x, size.y, stroke);



		// Update UI components
		for (OdGrUiComponent* control : childComponents) {
			control->onFrame(aContext);
		}

	}

};

#endif // OD_GR_UI_PANEL_H