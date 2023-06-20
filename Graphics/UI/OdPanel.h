#ifndef OD_GR_UI_PANEL_H
#define OD_GR_UI_PANEL_H

/**************************************************************************************
* OpenDraft:    GUI Panel Container Class                                             *
*-------------------------------------------------------------------------------------*
* Filename:     OdPanel.h                                                         *
* Contributors: James Hodgkins                                                        *
* Date:         June 18, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A GUI panel container class derived from OdComponent                            *
***************************************************************************************/



#include "System/OdCore.h"
#include "Graphics/UI/OdComponent.h"
#include "Graphics/OdDraw.h"


namespace OD
{
	namespace Graphics
	{

		class OdPanel : public OdComponent
		{
		protected:
			OdColour backColourHover;
			OdColour backColourActive;
			OdImage* backgroundImage;

		public:

			/**
			 * \brief Constructs a Panel object with the specified attributes.
			 * \param location (OdPoint) The position of the panel.
			 * \param size (OdPoint) The size of the panel.
			 * \param text (std::string) The text displayed on the panel.
			 */
			OdPanel(int aX = 0, int aY = 0, int aWidth = 150, int aHeight = 30)
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

			void setBackgroundImage(OdImage* aImage)
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

				//
				// Begin drawing Panel
				//
				OdDraw::Rect(aContext, getRelativeLocation().x, getRelativeLocation().y, size.x, size.y, backColour);
	


				if (backgroundImage != nullptr)
				{
					OdDraw::ResourceImage(aContext, getRelativeLocation().x, getRelativeLocation().y, size.x, size.y, backgroundImage);
				}

				OdDraw::RectStroke(aContext, getRelativeLocation().x, getRelativeLocation().y, size.x, size.y, stroke);



				// Update UI components
				for (OdComponent* control : childComponents) {
					control->onFrame(aContext);
				}

			}

		};

	} // namespace Graphics
}// namespace OD


#endif // OD_GR_UI_PANEL_H