#ifndef OD_DAT_DRAWINGDB_H
#define OD_DAT_DRAWINGDB_H

/**************************************************************************************
* OpenDraft:    Drawing Database Class                                                *
*-------------------------------------------------------------------------------------*
* Filename:     OdDrawingDb.h                                                         *
* Contributors: James Hodgkins                                                        *
* Date:         July 08, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Document class for OpenDraft framework.                                           *
***************************************************************************************/


#include <vector>
#include "DataManager/OdDbObject.h"




namespace OD
{

	namespace Geometry
	{
		class OdEntity;
	} // namespace Geometry


	namespace Data
	{

		class OdDrawingDb
		{
		private:
			std::vector<OdDbObject*> objects;
			
			int requestNewHandleId();

		public:

			// Constructor and Destructor
			OdDrawingDb();
			~OdDrawingDb();


			// Document Entities

			// Add DbObject Record
			void addRecord(OdDbObject* aObject);

			// Add Entity
			void addCreatedEntity(OD::Geometry::OdEntity* aObject);

			// Remove Entity
			void removeRecord(OdDbObject* aObject);

			// Get Entity
			OdDbObject* getRecordByHandle(int aHandle);

			OdDbObject* getRecordByIndex(int aIndex);

			// Get Entity Count
			int getRecordCount();

		};


	} // namespace OD::Data
} // namespace OD


#endif // !OD_DAT_DRAWINGDB_H
