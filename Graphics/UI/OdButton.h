#ifndef OD_GR_UI_BUTTON_H
#define OD_GR_UI_BUTTON_H

/**************************************************************************************
* OpenDraft:    GUI Button Class                                                      *
*-------------------------------------------------------------------------------------*
* Filename:     OdButton.h                                                            *
* Contributors: James Hodgkins                                                        *
* Date:         June 9, 2023                                                          *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A GUI button class derived from OdComponent                                       *
***************************************************************************************/


#include "System/OdCore.h"
#include "Graphics/UI/OdComponent.h"
#include "Graphics/OdDraw.h"

namespace OD
{
	namespace Graphics
	{

		class OdButton : public OdComponent
		{
		protected:
			OdColour backColourHover;
			OdColour foreColourHover;
			OdColour backColourActive;
			OdColour foreColourActive;
			OdImage* backgroundImage;

		public:

			/**
			 * \brief Constructs a Button object with the specified attributes.
			 * \param location (OdPoint) The position of the button.
			 * \param size (OdPoint) The size of the button.
			 * \param text (std::string) The text displayed on the button.
			 */
			OdButton(int aX = 0, int aY = 0, int aWidth = 150, int aHeight = 30, std::string aText = "Button")
			{
				location.x = aX;
				location.y = aY;
				size.x = aWidth;
				size.y = aHeight;

				backColour = OdColour::BACKGROUND1;
				backColourHover = OdColour::BACKGROUND2;
				backColourActive = OdColour::BACKGROUND2;

				foreColour = OdColour::WHITE;
				foreColourHover = OdColour::WHITE;
				foreColourActive = OdColour::PRIMARY;

				stroke = OdColour(0, 0, 0, 0);

				backgroundImage = nullptr;

				text = aText;
			}

			void setBackgroundImage(OdImage* aImage)
			{
				backgroundImage = aImage;
			}

			void clearBackgroundImage()
			{
				backgroundImage = nullptr;
			}

			void DrawButtonState(NVGcontext* aContext, float x, float y, const OdColour& backColour, const OdColour& foreColor)
			{
				OdDraw::Rect(aContext, x, y, size.x, size.y, backColour);
				OdDraw::Text(aContext, x, y, size.x, size.y, 14.0f, foreColor, text.c_str());
			}

			/**
			 * \brief Renders a default OD-GUI Button to a given NanoVG context (NVGContext) with the specified attributes.
			 * \param context (NVGcontext*) The nanovg pointer for rendering.
			 */
			virtual void const onFrame(NVGcontext* aContext) override
			{
				if (!enabled)
					return;

				if (aContext == nullptr)
					return;


				// Draw button state
				if (mouseDown)
					DrawButtonState(aContext, getRelativeLocation().x, getRelativeLocation().y, backColourActive, foreColourActive);
				
				else if (mouseOver)
					DrawButtonState(aContext, getRelativeLocation().x, getRelativeLocation().y - 1, backColourHover, foreColourHover);
				
				else
					DrawButtonState(aContext, getRelativeLocation().x, getRelativeLocation().y, backColour, foreColour);
				

				// Draw background image if set
				if (backgroundImage != nullptr)
					OdDraw::ResourceImage(aContext, getRelativeLocation().x, getRelativeLocation().y, size.x, size.y, backgroundImage);
				

				OdDraw::RectStroke(aContext, getRelativeLocation().x, getRelativeLocation().y, size.x, size.y, stroke);

			}

		};

	} // namespace Graphics
} // namespace OpenDraft

#endif // OD_GR_UI_BUTTON_H