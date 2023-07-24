#ifndef OD_GR_UI_PANEL_H
#define OD_GR_UI_PANEL_H

/**************************************************************************************
* OpenDraft:    GUI Panel Container Class                                             *
*-------------------------------------------------------------------------------------*
* Filename:     OdPanel.h                                                         *
* Contributors: James Hodgkins                                                        *
* Date:         June 18, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
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
			OdPanel(int aX = 0, int aY = 0, int aWidth = 150, int aHeight = 30);

			void setBackgroundImage(OdImage* aImage);
			void clearBackgroundImage();
			

			/**
			* \brief Renders a default OD-GUI Panel to a given NanoVG context (NVGContext) with the specified attributes.
			* \param context (NVGcontext*) The nanovg pointer for rendering.
			*/
			virtual void onFrame(NVGcontext* aContext) override;

			
			void actionEvents(GrInputMap* aInput);
			
		};

	} // namespace Graphics
}// namespace OD


#endif // !OD_GR_UI_PANEL_H