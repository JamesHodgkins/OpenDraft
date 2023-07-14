#ifndef OD_SY_LAYER_H
#define OD_SY_LAYER_H

/**************************************************************************************
* OpenDraft:    System Layer Class                                                    *
*-------------------------------------------------------------------------------------*
* Filename:     OdLayer.h                                                             *
* Contributors: James Hodgkins                                                        *
* Date:         July 10, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Entity layer class for OpenDraft framework.                                       *
***************************************************************************************/


#include "System/Entities/OdEntities.h"
#include "Graphics/UI/OdViewport.h"


namespace OD
{
	namespace Geometry
	{

		class OdTemplate : public OdEntity
		{

		public:

			// Properties
			std::string name;


			// Constructors and Destructors


			// Getters and Setters
			std::string getName() const
			{
				return name;
			}

			void setName(const std::string& aName)
			{
				name = aName;
			}


			// Methods



			//
			// Draw
			//
			void draw(NVGcontext* aContext, OdViewport* aVP) override
			{
				
			}

		};

	}
}



#endif // OD_SY_LAYER_H