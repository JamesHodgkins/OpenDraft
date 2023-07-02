#ifndef OD_SY_POINT_H
#define OD_SY_POINT_H

/**************************************************************************************
* OpenDraft:    System Point Entity Class                                             *
*-------------------------------------------------------------------------------------*
* Filename:     OdPoint.h                                                             *
* Contributors: James Hodgkins                                                        *
* Date:         July 02, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Point Entity class for OpenDraft framework.                                       *
***************************************************************************************/


#include "System/Entities/OdEntity.h"


using namespace OD::System;

namespace OD
{
	namespace Geometry
	{

		class OdPoint : public OdEntity {
		public:
			OdVector2 centre;


			//
			// Constructors
			//

			OdPoint()
			{
				centre = OdVector2(0, 0);
			}

			OdPoint(OdVector2 aCentre)
			{
				centre = aCentre;
			}

			OdPoint(float x, float y)
			{
				centre = OdVector2(x, y);
			}

			OdPoint(int x, int y)
			{
				centre = OdVector2(x, y);
			}


			//
			// Draw
			//

			void draw(NVGcontext* aContext) override
			{
				// To do: get line weight from layer
				//int lineWidthIndex = getLineWeight();
				//float lineWeight = OdSystem::getRegistryVariableByName("lineWidth");

				nvgBeginPath(aContext);
				nvgCircle(aContext, centre.x, centre.y, 0.8);
				nvgFillColor(aContext, nvgRGBA(255, 0, 0, 255));
				nvgFill(aContext);
			}

		};
	}
}


#endif // OD_SY_POINT_H