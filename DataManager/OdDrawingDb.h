#ifndef OD_DAT_DRAWINGDB_H
#define OD_DAT_DRAWINGDB_H

/**************************************************************************************
* OpenDraft:    Drawing Database Class                                                *
*-------------------------------------------------------------------------------------*
* Filename:     OdDrawingDb.h                                                         *
* Contributors: James Hodgkins                                                        *
* Date:         June 28, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Document class for OpenDraft framework.                                           *
***************************************************************************************/


#include <vector>
#include "DataManager/OdDbObject.h"


//using namespace OD::Geometry;


namespace OD
{
	namespace Data
	{

		class OdDrawingDb
		{
		private:
			std::vector<OdDbObject*> objects;

		public:

			// Constructor and Destructor
			OdDrawingDb();
			~OdDrawingDb();


			// Document Entities

			// Add Entity
			void AddCreatedEntity(OdDbObject* aObject);

			// Remove Entity
			void RemoveEntity(OdDbObject* aObject);

			// Get Entity
			OdDbObject* GetEntity(int aHandle);

			// Get Entity Count
			int GetEntityCount();

		};


	} // namespace OD::Data
} // namespace OD


#endif // OD_DAT_DRAWINGDB_H
