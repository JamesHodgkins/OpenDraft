#ifndef OD_GR_UI_BUTTON_H
#define OD_GR_UI_BUTTON_H

/**************************************************************************************
* OpenDraft:    GUI Button Class                                                      *
*-------------------------------------------------------------------------------------*
* Filename:     OdButton.h                                                            *
* Contributors: James Hodgkins                                                        *
* Date:         June 21, 2023                                                         *
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
			OdButton(int aX = 0, int aY = 0, int aWidth = 150, int aHeight = 30, std::string aText = "Button");

			/**
			 * \brief Sets the background image of the button. 
			 * \param aImage (OdImage*) The image to set as the background.
			 * \return void
			 */
			void setBackgroundImage(OdImage* aImage);

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
			void DrawButtonState(NVGcontext* aContext, float x, float y, const OdColour& backColour, const OdColour& foreColor);

			/**
			 * \brief Renders a default OD-GUI Button to a given NanoVG context (NVGContext) with the specified attributes.
			 * \param context (NVGcontext*) The nanovg pointer for rendering.
			 */
			virtual void const onFrame(NVGcontext* aContext) override;

		};

	} // namespace Graphics
} // namespace OpenDraft

#endif // OD_GR_UI_BUTTON_H