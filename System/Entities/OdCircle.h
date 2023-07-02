#ifndef OD_SY_CIRCLE_H
#define OD_SY_CIRCLE_H

/**************************************************************************************
* OpenDraft:    System Circle Entity Class                                            *
*-------------------------------------------------------------------------------------*
* Filename:     OdCircle.h                                                            *
* Contributors: James Hodgkins                                                        *
* Date:         June 27, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Circle Entity class for OpenDraft framework.                                      *
***************************************************************************************/


#include "System/Entities/OdEntity.h"


using namespace OD::System;

namespace OD
{
	namespace Geometry
	{

		class OdCircle : public OdEntity {
		public:
			OdVector2 centre;
			double radius;


			//
			// Constructors
			//
			
			OdCircle()
			{
				centre = OdVector2(0, 0);
				radius = 1;
			}

			OdCircle(OdVector2 aCentre, double aRadius)
			{
				centre = aCentre;
				radius = aRadius;
			}

			OdCircle(float x, float y, double aRadius)
			{
				centre = OdVector2(x, y);
				radius = aRadius;
			}

			OdCircle(int x, int y, double aRadius)
			{
				centre = OdVector2(x, y);
				radius = aRadius;
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
				nvgCircle(aContext, centre.x, centre.y, radius);
				nvgStrokeColor(aContext, nvgRGBA(255, 0, 0, 255));
				nvgStrokeWidth(aContext, 3);
				nvgStroke(aContext);
			}

			//
			// Calculated Properties
			//

			// Area
			double getArea()
			{
				return OD_PI * radius * radius;
			}

			// Circumference
			double getCircumference()
			{
				return 2 * OD_PI * radius;
			}

			// Diameter
			double getDiameter()
			{
				return 2 * radius;
			}

			// Radius
			double getRadius()
			{
				return radius;
			}

		};
	}
}


#endif // OD_SY_CIRCLE_H