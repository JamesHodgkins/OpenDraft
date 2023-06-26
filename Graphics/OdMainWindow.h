#ifndef OD_GR_MAIN_WINDOW_H
#define OD_GR_MAIN_WINDOW_H

/**************************************************************************************
* OpenDraft:    Main Application Window Class                                         *
*-------------------------------------------------------------------------------------*
* Filename:     OdColour.h                                                            *
* Contributors: James Hodgkins                                                        *
* Date:         June 26, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A class for the main application window                                           *
***************************************************************************************/



#include "Graphics/OdGraphics.h"


namespace OD
{
	namespace Graphics
	{

		class OdMainWindow : public OdWindow
		{
		protected:

			// Load required resources
			void loadResources();

		public:

			// Constructor
			OdMainWindow(int aWidth, int aHeight, const char* aTitle);

			// Destructor
			~OdMainWindow();

			// Initialize the window and UI components
			void initialise() override;

			// Render the window and UI components
			virtual void onFrame(NVGcontext* NULLREF = nullptr, OdComponent* aParent = nullptr);
		};

	} // namespace Graphics
}// namespace OD

#endif // OD_GR_MAIN_WINDOW_H