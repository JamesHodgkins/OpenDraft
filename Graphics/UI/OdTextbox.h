#ifndef OD_GR_UI_TEXTBOX_H
#define OD_GR_UI_TEXTBOX_H

/**************************************************************************************
* OpenDraft:    GUI Textbox Class                                                     *
*-------------------------------------------------------------------------------------*
* Filename:     OdTextbox.h                                                           *
* Contributors: James Hodgkins                                                        *
* Date:         July 24, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A GUI textbox input class derived from OdComponent                                *
***************************************************************************************/



#include <unordered_map>
#include "System/OdCore.h"
#include "Graphics/UI/OdComponent.h"
#include "Graphics/OdDraw.h"


namespace OD
{
	namespace Graphics
	{
		class OdTextbox : public OdComponent
		{
		protected:
			// Constants
			const int CURSOR_BLINK_FRAME_COUNT = 25;

			// Properties
			bool isActive;
			float fontSize;
			int cursorIndex;
			int cursorBlinkTimer;


		public:
			OdTextbox();
			OdTextbox(int aX, int aY, int aWidth, int aHeight, std::string aText);
			OdTextbox(OdVector2 aLocation, std::string aText);


			int calculateCursorPosition(NVGcontext* aContext, int aIndex);


			virtual void onFrame(NVGcontext* aContext) override;
			void actionEvents(GrInputMap* aInput) override;

		};

	}
}

#endif // !OD_GR_UI_TEXTBOX_H