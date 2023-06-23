#ifndef OD_GR_UI_LABEL_CPP
#define OD_GR_UI_LABEL_CPP

/**************************************************************************************
* OpenDraft:    GUI Label Class                                                       *
*-------------------------------------------------------------------------------------*
* Filename:     OdLabel.cpp                                                           *
* Contributors: James Hodgkins                                                        *
* Date:         June 23, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A GUI label class derived by OdComponent                                          *
***************************************************************************************/



#include "Graphics/UI/OdLabel.h"


namespace OD
{
	namespace Graphics
	{

			/**
			 * \brief Constructs a Label object with the specified attributes.
			 * \param location (OdPoint) The position of the Label.
			 * \param size (OdPoint) The size of the Label.
			 * \param text (std::string) The text displayed on the Label.
			 */
			OdLabel::OdLabel(int aX, int aY, int aWidth, int aHeight, std::string aText)
			{
				location.x = aX;
				location.y = aY;
				size.x = aWidth;
				size.y = aHeight;

				backColour = OdColour(255, 0, 0, 255);
				foreColour = OdColour(255, 255, 255, 200);

				text = aText;
			}


			/**
			 * \brief Renders a default OD-GUI Label to a given NanoVG context (NVGContext) with the specified attributes.
			 * \param context (NVGcontext*) The nanovg pointer for rendering.
			 */
			void OdLabel::onFrame(NVGcontext* aContext)
			{
				if (aContext == nullptr)
					return;

				//
				// Begin drawing Label
				//
				OdDraw::Rect(aContext, getRelativeLocation().x, getRelativeLocation().y, size.x, size.y, backColour);
				OdDraw::Text(aContext, getRelativeLocation().x, getRelativeLocation().y, size.x, size.y, 14.0f, foreColour, text.c_str());

			}


	} // namespace Graphics
} // namespace OD

#endif // OD_GR_UI_LABEL_CPP