#ifndef OD_GR_UI_BUTTON_H
#define OD_GR_UI_BUTTON_H

/**************************************************************************************
* OpenDraft:    GUI Button Class                                                      *
*-------------------------------------------------------------------------------------*
* Filename:     OdButton.h                                                            *
* Contributors: James Hodgkins                                                        *
* Date:         June 21, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
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
			OdImage* backgroundImage;
			float fontSize;
			float padding;

		public:
			OdColour backColourHover;
			OdColour foreColourHover;
			OdColour backColourActive;
			OdColour foreColourActive;

			/**
			 * \brief Constructs a Button object with the specified attributes.
			 * \param location (OdPoint) The position of the button.
			 * \param size (OdPoint) The size of the button.
			 * \param text (std::string) The text displayed on the button.
			 */
			OdButton(const int aX = 0, const int aY = 0, const int aWidth = 150, const int aHeight = 30, const std::string aText = "Button");

			/**
			 * \brief Sets the background image of the button. 
			 * \param aImage (OdImage*) The image to set as the background.
			 * \return void
			 */
			void setBackgroundImage(OdImage* aImage);

			/**
			 * \brief Sets font size.
			 * \param aFontSize (int) The font size to set.
			 * \return void
			 */
			void setFontSize(int aFontSize) { fontSize = aFontSize; }

			/**
			 * \brief Clears the background image of the button.
			 * \return void
			 */
			void clearBackgroundImage();

			/**
			 * \brief Draws the button to the specified NanoVG context (NVGContext) with the specified attributes.
			 * \param aContext (NVGcontext*) The nanovg pointer for rendering.
			 * \param x The x position of the button.
			 * \param y The y position of the button.
			 * \param backColour The background colour of the button.
			 * \param foreColor The foreground colour of the button.
			 */
			void DrawButtonState(NVGcontext* aContext, const float x, const float y, const OdColour& backColour, const OdColour& foreColour);

			/**
			 * \brief Renders a default OD-GUI Button to a given NanoVG context (NVGContext) with the specified attributes.
			 * \param context (NVGcontext*) The nanovg pointer for rendering.
			 */
			virtual void onFrame(NVGcontext* aContext) override;


			void actionEvents(GrInputMap* aInput) override;

		};

	} // namespace Graphics
} // namespace OpenDraft

#endif // !OD_GR_UI_BUTTON_H
