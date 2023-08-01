#ifndef OD_GR_VIEWPORT_H
#define OD_GR_VIEWPORT_H

/**************************************************************************************
* OpenDraft:    Main Graphics Header                                                  *
*-------------------------------------------------------------------------------------*
* Filename:     OdGraphics.h                                                          *
* Contributors: James Hodgkins                                                        *
* Date:         June 28, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Header file collates all necessary graphics                                       *
***************************************************************************************/

#include <vector>

#define GLEW_STATIC					// Use static glew library (glew32s.lib)
#include "GL/glew.h"				// Include glew
#include "glfw/glfw3.h"				// Include glfw3
#include "nanovg.h"					// Include core nanovg library

#include "System/OdCore.h"
#include "System/Entities/OdEntity.h"
#include "System/Entities/OdEntities.h"
#include "Graphics/OdComponents.h"
#include "Graphics/OdDraw.h"


using namespace OD::Geometry;

namespace OD
{
	namespace Graphics
	{

		class OdViewport : public OdComponent
		{
		private:
			bool dragState = false;
			OdVector2 dragStart;
			OdVector2 cursorPosition;
			float scale = 1.0f;

		protected:
			OdVector2 position;

		public:
			// Properties
			std::vector<OdEntity*>* entities;

			// Constructor
			OdViewport(int aX, int aY, int aWidth, int aHeight);

			void onFrame(NVGcontext* aContext);
			void actionEvents(GrInputMap* aInput);

			void translatePosition(float aX, float aY);
			void setPosition(float aX, float aY);
			OdVector2 getPosition();
			void setScale(float aScale, int aMouseX, int aMouseY);
			void scaleByAmount(float aAmount, int aMouseX, int aMouseY);
			float getScale();

			// User input methods
			OdVector2 getCoordinatesAtScreenPosition(float x, float y);
			OdVector2 getScreenPositionAtCoordinates(float x, float y);

			// Check entities
			void updateEntityStates(OdVector2 aPoint);

		};

	} // namespace OD::Graphics
}// namespace OD

#endif // !OD_GR_GRAPHICS_H
