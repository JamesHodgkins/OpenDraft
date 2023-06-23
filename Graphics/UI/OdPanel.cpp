#ifndef OD_GR_UI_PANEL_CPP
#define OD_GR_UI_PANEL_CPP

/**************************************************************************************
* OpenDraft:    GUI Panel Container Class                                             *
*-------------------------------------------------------------------------------------*
* Filename:     OdPanel.cpp                                                           *
* Contributors: James Hodgkins                                                        *
* Date:         June 23, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A GUI panel container class derived from OdComponent                              *
***************************************************************************************/



#include "Graphics/UI/OdPanel.h"


namespace OD
{
	namespace Graphics
	{

		/**
		* \brief Constructs a Panel object with the specified attributes.
		* \param location (OdPoint) The position of the panel.
		* \param size (OdPoint) The size of the panel.
		* \param text (std::string) The text displayed on the panel.
		*/
		OdPanel::OdPanel(int aX, int aY, int aWidth, int aHeight)
		{
			location.x = aX;
			location.y = aY;
			size.x = aWidth;
			size.y = aHeight;

			backColour = OdColour::BACKGROUND1;
			backColourHover = OdColour::BACKGROUND2;
			backColourActive = OdColour(0, 0, 255, 255);
			stroke = OdColour(0, 0, 0, 0);
			foreColour = OdColour(255, 255, 255, 200);

			backgroundImage = nullptr;

			text = "";
		}

		void OdPanel::setBackgroundImage(OdImage* aImage)
		{
			backgroundImage = aImage;
		}

		void OdPanel::clearBackgroundImage()
		{
			backgroundImage = nullptr;
		}


		/**
		* \brief Renders a default OD-GUI Panel to a given NanoVG context (NVGContext) with the specified attributes.
		* \param context (NVGcontext*) The nanovg pointer for rendering.
		*/
		void OdPanel::onFrame(NVGcontext* aContext)
		{
			if (!enabled)
				return;

			if (aContext == nullptr)
				return;

			//
			// Begin drawing Panel
			//
			OdDraw::Rect(aContext, getRelativeLocation().x, getRelativeLocation().y, size.x, size.y, backColour);



			if (backgroundImage != nullptr)
			{
				OdDraw::ResourceImage(aContext, getRelativeLocation().x, getRelativeLocation().y, size.x, size.y, backgroundImage);
			}


			OdDraw::RectStroke(aContext, getRelativeLocation().x, getRelativeLocation().y, size.x, size.y, stroke);


			// Draw child UI components
			drawChildComponents(aContext);

		}

		

	} // namespace Graphics
}// namespace OD


#endif // OD_GR_UI_PANEL_CPP