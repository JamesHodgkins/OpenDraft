#ifndef OD_GR_MAIN_WINDOW_H
#define OD_GR_MAIN_WINDOW_H

/**************************************************************************************
* OpenDraft:    Main Application Window Class                                         *
*-------------------------------------------------------------------------------------*
* Filename:     OdColour.h                                                            *
* Contributors: James Hodgkins                                                        *
* Date:         July 08, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
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

			// Properties
			OdViewport* viewport;

		public:

			// Constructor
			OdMainWindow(int aWidth, int aHeight, const char* aTitle);

			// Destructor
			~OdMainWindow() override;

			// Delete copy constructor
			OdMainWindow(const OdMainWindow& aOther) = delete;

			// Delete move constructor
			OdMainWindow(OdMainWindow&& aOther) = delete;

			bool operator==(const OdMainWindow& other) const = default;

			// Initialize the window and UI components
			void initialise() override;

			// Render the window and UI components
			virtual void onFrame(NVGcontext* NULLREF = nullptr) override;

			// Action window events
			void actionEvents(GrInputMap* aInput);

		};

	} // namespace Graphics
}// namespace OD

#endif // !OD_GR_MAIN_WINDOW_H